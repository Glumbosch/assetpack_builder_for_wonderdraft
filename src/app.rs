use std::path::PathBuf;

use eframe::egui::{
    self, pos2, vec2, Align2, Color32, ColorImage, ComboBox, CursorIcon, FontId, Key,
    Modifiers, PointerButton, Pos2, Rect, Sense, Stroke, StrokeKind, TextureOptions, Vec2,
};
use image::{imageops, RgbaImage};

use crate::{
    export::{
        export_pack, load_project, sanitize_file_stem, save_project, LoadedProject,
    },
    image_ops::{
        apply_brush_line, apply_brush_stamp, crop_rgba, load_oriented_rgba,
        remove_picked_color, smart_edge_remove, soften_alpha, threshold_alpha, BrushMode,
    },
    model::{
        AssetKind, CropRegion, DrawMode, SourceImage, SpriteAsset, ThemeDraft,
    },
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MainTab {
    Assets,
    Themes,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AssetView {
    Crop,
    Sprite,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CropTool {
    SelectMove,
    Draw,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SpriteTool {
    Erase,
    Restore,
    PickColor,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CropHandle {
    NorthWest,
    North,
    NorthEast,
    East,
    SouthEast,
    South,
    SouthWest,
    West,
}

#[derive(Debug, Clone)]
enum CropDrag {
    Move {
        crop_id: u64,
        start: (f32, f32),
        original: CropRegion,
    },
    Resize {
        crop_id: u64,
        handle: CropHandle,
        start: (f32, f32),
        original: CropRegion,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SpriteOverlayDrag {
    Pivot,
    Radius,
}

#[derive(Debug, Clone, Copy)]
struct SpriteOverlay {
    pivot: Pos2,
    radius_screen: f32,
    scale: f32,
}

const SPRITE_PIVOT_HIT_RADIUS: f32 = 24.0;

pub struct WonderdraftAssetStudio {
    pack_name: String,
    sources: Vec<SourceImage>,
    sprites: Vec<SpriteAsset>,
    themes: Vec<ThemeDraft>,
    next_id: u64,

    selected_source: Option<u64>,
    selected_crop: Option<u64>,
    selected_sprite: Option<u64>,
    selected_theme: usize,

    main_tab: MainTab,
    asset_view: AssetView,
    crop_tool: CropTool,
    sprite_tool: SpriteTool,

    crop_drag_start: Option<(f32, f32)>,
    crop_drag_current: Option<(f32, f32)>,
    crop_drag: Option<CropDrag>,

    sprite_overlay_drag: Option<SpriteOverlayDrag>,
    sprite_zoom: f32,
    sprite_pan: Vec2,

    brush_size: f32,
    tolerance: u8,
    alpha_blur_radius: u32,
    picked_color: [u8; 3],
    last_brush_point: Option<(f32, f32)>,
    brush_history_active: bool,

    project_path: Option<PathBuf>,
    status: String,
}

impl Default for WonderdraftAssetStudio {
    fn default() -> Self {
        Self {
            pack_name: "MyFantasyPack".to_owned(),
            sources: Vec::new(),
            sprites: Vec::new(),
            themes: vec![ThemeDraft::default()],
            next_id: 1,
            selected_source: None,
            selected_crop: None,
            selected_sprite: None,
            selected_theme: 0,
            main_tab: MainTab::Assets,
            asset_view: AssetView::Crop,
            crop_tool: CropTool::SelectMove,
            sprite_tool: SpriteTool::Erase,
            crop_drag_start: None,
            crop_drag_current: None,
            crop_drag: None,
            sprite_overlay_drag: None,
            sprite_zoom: 1.0,
            sprite_pan: Vec2::ZERO,
            brush_size: 24.0,
            tolerance: 24,
            alpha_blur_radius: 2,
            picked_color: [255, 255, 255],
            last_brush_point: None,
            brush_history_active: false,
            project_path: None,
            status: "Import images or drag files into the window.".to_owned(),
        }
    }
}

impl WonderdraftAssetStudio {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        cc.egui_ctx.set_visuals(egui::Visuals::dark());
        cc.egui_ctx.all_styles_mut(|style| {
            style.spacing.button_padding = vec2(10.0, 10.0);
            style.spacing.interact_size.y = 36.0;
        });
        Self::default()
    }

    fn alloc_id(&mut self) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    fn source_index(&self, id: u64) -> Option<usize> {
        self.sources.iter().position(|source| source.id == id)
    }

    fn sprite_index(&self, id: u64) -> Option<usize> {
        self.sprites.iter().position(|sprite| sprite.id == id)
    }

    fn selected_source_index(&self) -> Option<usize> {
        self.selected_source.and_then(|id| self.source_index(id))
    }

    fn selected_sprite_index(&self) -> Option<usize> {
        self.selected_sprite.and_then(|id| self.sprite_index(id))
    }

    fn import_dialog(&mut self) {
        if let Some(paths) = rfd::FileDialog::new()
            .add_filter("Images", &["png", "jpg", "jpeg", "webp", "bmp", "tif", "tiff"])
            .pick_files()
        {
            self.import_paths(paths);
        }
    }

    fn import_paths<I>(&mut self, paths: I)
    where
        I: IntoIterator<Item = PathBuf>,
    {
        let mut imported = 0usize;
        let mut errors = Vec::new();
        for path in paths {
            if !path.is_file() {
                continue;
            }
            match load_oriented_rgba(&path) {
                Ok(rgba) => {
                    let source_id = self.alloc_id();
                    self.sources.push(SourceImage {
                        id: source_id,
                        path,
                        image: rgba,
                        crops: Vec::new(),
                        exif_orientation_applied: true,
                        quarter_turns: 0,
                        texture: None,
                    });
                    self.selected_source = Some(source_id);
                    self.selected_crop = None;
                    self.asset_view = AssetView::Crop;
                    self.crop_tool = CropTool::Draw;
                    imported += 1;
                }
                Err(error) => errors.push(format!("{}: {error}", path.display())),
            }
        }
        self.status = if errors.is_empty() {
            format!("Imported {imported} image(s). Draw one or more crop regions.")
        } else {
            format!(
                "Imported {imported} image(s); {} file(s) failed: {}",
                errors.len(),
                errors.join(" | ")
            )
        };
    }

    fn rotate_selected_source_clockwise(&mut self) {
        let Some(source_index) = self.selected_source_index() else {
            return;
        };

        let source_id = self.sources[source_index].id;
        let old_height = self.sources[source_index].image.height();
        let source = &mut self.sources[source_index];
        source.image = imageops::rotate90(&source.image);
        source.quarter_turns = (source.quarter_turns + 1) % 4;
        source.texture = None;
        for crop in &mut source.crops {
            rotate_crop_clockwise(crop, old_height);
        }

        for sprite in self
            .sprites
            .iter_mut()
            .filter(|sprite| sprite.source_id == source_id)
        {
            sprite.original = imageops::rotate90(&sprite.original);
            sprite.working = imageops::rotate90(&sprite.working);
            (sprite.offset_x, sprite.offset_y) = (sprite.offset_y, -sprite.offset_x);
            sprite.texture_dirty = true;
            sprite.undo.clear();
            sprite.redo.clear();
        }

        self.crop_drag_start = None;
        self.crop_drag_current = None;
        self.crop_drag = None;
        self.status = "Rotated the source and its crops 90° clockwise.".to_owned();
    }

    fn handle_dropped_files(&mut self, ctx: &egui::Context) {
        let dropped_files = ctx.input(|input| input.raw.dropped_files.clone());

        if dropped_files.is_empty() {
            return;
        }

        let paths = dropped_files
            .into_iter()
            .filter_map(|file| file.path)
            .collect::<Vec<_>>();

        if !paths.is_empty() {
            self.import_paths(paths);
        }
    }

    fn paint_drop_overlay(&self, ctx: &egui::Context) {
        let has_hovered_files = ctx.input(|input| !input.raw.hovered_files.is_empty());
        if !has_hovered_files {
            return;
        }

        let painter = ctx.layer_painter(egui::LayerId::new(
            egui::Order::Foreground,
            egui::Id::new("file_drop_overlay"),
        ));
        let rect = ctx.input(|input| input.content_rect()).shrink(24.0);
        painter.rect_filled(
            rect,
            12.0,
            Color32::from_rgba_unmultiplied(20, 30, 40, 220),
        );
        painter.rect_stroke(
            rect,
            12.0,
            Stroke::new(4.0, Color32::LIGHT_BLUE),
            StrokeKind::Inside,
        );
        painter.text(
            rect.center(),
            Align2::CENTER_CENTER,
            "Drop images anywhere to import them",
            FontId::proportional(28.0),
            Color32::WHITE,
        );
    }

    fn handle_shortcuts(&mut self, ctx: &egui::Context) {
        if self.main_tab != MainTab::Assets || ctx.egui_wants_keyboard_input() {
            return;
        }

        if self.asset_view == AssetView::Crop {
            self.handle_crop_shortcuts(ctx);
            return;
        }

        let ctrl_shift = Modifiers {
            ctrl: true,
            shift: true,
            ..Default::default()
        };

        let redo = ctx.input_mut(|input| input.consume_key(ctrl_shift, Key::Z));
        let undo = if redo {
            false
        } else {
            ctx.input_mut(|input| {
                input.consume_key(
                    Modifiers {
                        ctrl: true,
                        ..Default::default()
                    },
                    Key::Z,
                )
            })
        };

        let Some(sprite_index) = self.selected_sprite_index() else {
            return;
        };

        if redo {
            if self.sprites[sprite_index].redo() {
                self.status = "Redid sprite edit.".to_owned();
            }
        } else if undo && self.sprites[sprite_index].undo() {
            self.status = "Undid sprite edit.".to_owned();
        }
    }

    fn handle_crop_shortcuts(&mut self, ctx: &egui::Context) {
        let (shift, left, right, up, down) = ctx.input_mut(|input| {
            if input.modifiers.ctrl || input.modifiers.alt || input.modifiers.command {
                return (false, false, false, false, false);
            }
            let shift = input.modifiers.shift;
            let modifiers = Modifiers {
                shift,
                ..Default::default()
            };
            let left = input.consume_key(modifiers, Key::ArrowLeft)
                || input.consume_key(modifiers, Key::A);
            let right = input.consume_key(modifiers, Key::ArrowRight)
                || input.consume_key(modifiers, Key::D);
            let up = input.consume_key(modifiers, Key::ArrowUp)
                || input.consume_key(modifiers, Key::W);
            let down = input.consume_key(modifiers, Key::ArrowDown)
                || input.consume_key(modifiers, Key::S);
            (shift, left, right, up, down)
        });

        if !(left || right || up || down) {
            return;
        }
        let Some(source_index) = self.selected_source_index() else {
            return;
        };
        let Some(crop_id) = self.selected_crop else {
            return;
        };
        let (source_w, source_h) = self.sources[source_index].image.dimensions();
        let Some(crop) = self.sources[source_index]
            .crops
            .iter_mut()
            .find(|crop| crop.id == crop_id)
        else {
            return;
        };

        adjust_crop_with_keys(crop, source_w, source_h, shift, left, right, up, down);
    }

    fn new_project(&mut self) {
        *self = Self::default();
        self.status = "Created a new project.".to_owned();
    }

    fn save_project_action(&mut self) {
        let path = match self.project_path.clone() {
            Some(path) => path,
            None => match rfd::FileDialog::new()
                .add_filter("Wonderdraft Asset Studio project", &["wdassetproj"])
                .set_file_name("asset-pack.wdassetproj")
                .save_file()
            {
                Some(path) => ensure_extension(path, "wdassetproj"),
                None => return,
            },
        };
        match save_project(
            &path,
            &self.pack_name,
            self.next_id,
            &self.sources,
            &self.sprites,
            &self.themes,
        ) {
            Ok(()) => {
                self.project_path = Some(path.clone());
                self.status = format!("Saved project to {}", path.display());
            }
            Err(error) => self.status = format!("Could not save project: {error:#}"),
        }
    }

    fn save_project_as_action(&mut self) {
        let previous = self.project_path.take();
        self.save_project_action();
        if self.project_path.is_none() {
            self.project_path = previous;
        }
    }

    fn open_project_action(&mut self) {
        let Some(path) = rfd::FileDialog::new()
            .add_filter("Wonderdraft Asset Studio project", &["wdassetproj"])
            .pick_file()
        else {
            return;
        };
        match load_project(&path) {
            Ok(project) => self.install_loaded_project(path, project),
            Err(error) => self.status = format!("Could not open project: {error:#}"),
        }
    }

    fn install_loaded_project(&mut self, path: PathBuf, project: LoadedProject) {
        self.pack_name = project.pack_name;
        self.next_id = project.next_id;
        self.sources = project.sources;
        self.sprites = project.sprites;
        self.themes = if project.themes.is_empty() {
            vec![ThemeDraft::default()]
        } else {
            project.themes
        };
        self.selected_source = self.sources.first().map(|source| source.id);
        self.selected_crop = self
            .sources
            .first()
            .and_then(|source| source.crops.first())
            .map(|crop| crop.id);
        self.selected_sprite = self.sprites.first().map(|sprite| sprite.id);
        self.selected_theme = 0;
        self.main_tab = MainTab::Assets;
        self.asset_view = AssetView::Crop;
        self.crop_tool = CropTool::SelectMove;
        self.crop_drag_start = None;
        self.crop_drag_current = None;
        self.sprite_zoom = 1.0;
        self.sprite_pan = Vec2::ZERO;
        self.sprite_overlay_drag = None;
        self.crop_drag = None;
        self.project_path = Some(path.clone());
        self.status = if project.warnings.is_empty() {
            format!("Opened {}", path.display())
        } else {
            format!(
                "Opened {} with warnings: {}",
                path.display(),
                project.warnings.join(" | ")
            )
        };
    }

    fn export_action(&mut self) {
        let Some(root) = rfd::FileDialog::new().pick_folder() else {
            return;
        };
        match export_pack(&root, &self.pack_name, &self.sprites, &self.themes) {
            Ok(report) => {
                let mut status = format!(
                    "Exported {} PNG(s), {} .wonderdraft_symbols file(s), and {} theme(s) into {}.",
                    report.png_files,
                    report.metadata_files,
                    report.theme_files,
                    root.display()
                );
                if !report.warnings.is_empty() {
                    status.push_str(" Warnings: ");
                    status.push_str(&report.warnings.join(" | "));
                }
                self.status = status;
            }
            Err(error) => self.status = format!("Export failed: {error:#}"),
        }
    }

    fn extract_selected_crop(&mut self) {
        let Some(source_index) = self.selected_source_index() else {
            self.status = "Select a source image first.".to_owned();
            return;
        };
        let Some(crop_id) = self.selected_crop else {
            self.status = "Select or draw a crop first.".to_owned();
            return;
        };

        let crop_index = self.sources[source_index]
            .crops
            .iter()
            .position(|crop| crop.id == crop_id);
        let Some(crop_index) = crop_index else {
            return;
        };

        if let Some(existing_id) = self.sources[source_index].crops[crop_index].sprite_id {
            self.selected_sprite = Some(existing_id);
            self.asset_view = AssetView::Sprite;
            self.status = "This crop already has a sprite; selected it instead.".to_owned();
            return;
        }

        let crop = self.sources[source_index].crops[crop_index].clone();
        let source_id = self.sources[source_index].id;
        let source_stem = self.sources[source_index]
            .path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("sprite")
            .to_owned();
        let extracted = crop_rgba(
            &self.sources[source_index].image,
            crop.x,
            crop.y,
            crop.width,
            crop.height,
        );
        let sprite_id = self.alloc_id();
        let sequence = self.sprites.len() + 1;
        let name = format!("{} {}", humanize(&source_stem), sequence);
        let file_stem = sanitize_file_stem(&format!("{}_{}", source_stem, sequence));
        let radius = (extracted.width().max(extracted.height()) / 2).max(1) as i32;
        self.sprites.push(SpriteAsset {
            id: sprite_id,
            source_id,
            crop_id,
            original: extracted.clone(),
            working: extracted,
            name,
            file_stem,
            kind: AssetKind::Symbols,
            category: "Uncategorized".to_owned(),
            draw_mode: DrawMode::Normal,
            radius,
            offset_x: 0,
            offset_y: 0,
            texture: None,
            texture_dirty: true,
            undo: Vec::new(),
            redo: Vec::new(),
        });
        self.sources[source_index].crops[crop_index].sprite_id = Some(sprite_id);
        self.selected_sprite = Some(sprite_id);
        self.asset_view = AssetView::Sprite;
        self.sprite_zoom = 1.0;
        self.sprite_pan = Vec2::ZERO;
        self.status = "Extracted crop as a new independently editable sprite.".to_owned();
    }

    fn copy_selected_crop(&mut self) {
        let Some(source_index) = self.selected_source_index() else {
            return;
        };
        let Some(crop_id) = self.selected_crop else {
            return;
        };

        let Some(original) = self.sources[source_index]
            .crops
            .iter()
            .find(|crop| crop.id == crop_id)
            .cloned()
        else {
            return;
        };

        let image_width = self.sources[source_index].image.width();
        let image_height = self.sources[source_index].image.height();
        let mut copy = original;
        copy.id = self.alloc_id();
        copy.sprite_id = None;
        copy.x = copy
            .x
            .saturating_add(10)
            .min(image_width.saturating_sub(copy.width));
        copy.y = copy
            .y
            .saturating_add(10)
            .min(image_height.saturating_sub(copy.height));

        let new_id = copy.id;
        self.sources[source_index].crops.push(copy);
        self.selected_crop = Some(new_id);
        self.status = "Copied crop. The copy has no extracted sprite yet.".to_owned();
    }

    fn extract_all_crops(&mut self) {
        let Some(source_index) = self.selected_source_index() else {
            self.status = "Select a source image first.".to_owned();
            return;
        };

        let crop_ids: Vec<u64> = self.sources[source_index]
            .crops
            .iter()
            .filter(|crop| crop.sprite_id.is_none())
            .map(|crop| crop.id)
            .collect();

        if crop_ids.is_empty() {
            self.status = "There are no unextracted crops.".to_owned();
            return;
        }

        let mut extracted = 0usize;
        let mut last_sprite = None;
        for crop_id in crop_ids {
            self.selected_crop = Some(crop_id);
            let previous_count = self.sprites.len();
            self.extract_selected_crop();
            if self.sprites.len() > previous_count {
                extracted += 1;
                last_sprite = self.selected_sprite;
            }
        }

        if let Some(sprite_id) = last_sprite {
            self.selected_sprite = Some(sprite_id);
            self.asset_view = AssetView::Sprite;
            self.sprite_zoom = 1.0;
            self.sprite_pan = Vec2::ZERO;
        }
        self.status = format!("Extracted {extracted} crop(s) as sprites.");
    }

    fn delete_selected_crop(&mut self) {
        let Some(source_index) = self.selected_source_index() else {
            return;
        };
        let Some(crop_id) = self.selected_crop else {
            return;
        };
        if let Some(index) = self.sources[source_index]
            .crops
            .iter()
            .position(|crop| crop.id == crop_id)
        {
            if self.sources[source_index].crops[index].sprite_id.is_some() {
                self.status = "Delete the extracted sprite first, then delete its crop.".to_owned();
                return;
            }
            self.sources[source_index].crops.remove(index);
            self.selected_crop = self.sources[source_index].crops.first().map(|crop| crop.id);
            self.status = "Deleted crop.".to_owned();
        }
    }

    fn delete_selected_sprite(&mut self) {
        let Some(sprite_id) = self.selected_sprite else {
            return;
        };
        if let Some(index) = self.sprite_index(sprite_id) {
            let source_id = self.sprites[index].source_id;
            let crop_id = self.sprites[index].crop_id;
            self.sprites.remove(index);
            if let Some(source_index) = self.source_index(source_id) {
                if let Some(crop) = self.sources[source_index]
                    .crops
                    .iter_mut()
                    .find(|crop| crop.id == crop_id)
                {
                    crop.sprite_id = None;
                }
            }
            self.selected_sprite = self.sprites.first().map(|sprite| sprite.id);
            if self.selected_sprite.is_none() {
                self.asset_view = AssetView::Crop;
            }
            self.status = "Deleted sprite. Its crop remains and can be extracted again.".to_owned();
        }
    }

    fn top_bar(&mut self, root_ui: &mut egui::Ui) {
        egui::Panel::top("top_bar").show(root_ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                if ui.button("New").clicked() {
                    self.new_project();
                }
                if ui.button("Open project").clicked() {
                    self.open_project_action();
                }
                if ui.button("Save").clicked() {
                    self.save_project_action();
                }
                if ui.button("Save as…").clicked() {
                    self.save_project_as_action();
                }
                ui.separator();
                if ui.button("Import images").clicked() {
                    self.import_dialog();
                }
                ui.separator();
                ui.selectable_value(&mut self.main_tab, MainTab::Assets, "Assets");
                ui.selectable_value(&mut self.main_tab, MainTab::Themes, "Themes");
                ui.separator();
                ui.label("Pack name:");
                ui.add(egui::TextEdit::singleline(&mut self.pack_name).desired_width(180.0));
            });
        });
    }

    fn status_bar(&mut self, root_ui: &mut egui::Ui) {
        egui::Panel::bottom("status_bar").show(root_ui, |ui| {
            ui.add_space(6.0);
            let row_size = vec2(ui.available_width(), ui.spacing().interact_size.y);
            ui.allocate_ui_with_layout(
                row_size,
                egui::Layout::right_to_left(egui::Align::Center),
                |ui| {
                    ui.add_space(8.0);
                    if ui.button("Export Wonderdraft pack").clicked() {
                        self.export_action();
                    }
                    ui.separator();
                    ui.label(&self.status);
                },
            );
            ui.add_space(6.0);
        });
    }

    fn assets_ui(&mut self, root_ui: &mut egui::Ui) {
        self.asset_left_panel(root_ui);
        self.asset_right_panel(root_ui);
        egui::CentralPanel::default().show(root_ui, |ui| match self.asset_view {
            AssetView::Crop => self.crop_canvas(ui),
            AssetView::Sprite => self.sprite_canvas(ui),
        });
    }

    fn asset_left_panel(&mut self, root_ui: &mut egui::Ui) {
        egui::Panel::left("asset_left")
            .resizable(true)
            .default_size(250.0)
            .show(root_ui, |ui| {
                ui.heading("Sources and sprites");
                ui.horizontal(|ui| {
                    if ui.button("Import").clicked() {
                        self.import_dialog();
                    }
                    ui.label("Drag-and-drop is supported.");
                });
                ui.separator();

                egui::ScrollArea::vertical().show(ui, |ui| {
                    ui.strong("Source images");
                    for source in &self.sources {
                        let selected = self.selected_source == Some(source.id)
                            && self.asset_view == AssetView::Crop;
                        if ui.selectable_label(selected, source.display_name()).clicked() {
                            self.selected_source = Some(source.id);
                            self.selected_crop = source.crops.first().map(|crop| crop.id);
                            self.asset_view = AssetView::Crop;
                        }
                    }

                    if let Some(source_index) = self.selected_source_index() {
                        ui.indent("crops", |ui| {
                            for (index, crop) in self.sources[source_index].crops.iter().enumerate() {
                                let suffix = if crop.sprite_id.is_some() { " ✓" } else { "" };
                                if ui
                                    .selectable_label(
                                        self.selected_crop == Some(crop.id)
                                            && self.asset_view == AssetView::Crop,
                                        format!(
                                            "Crop {} — {}×{}{}",
                                            index + 1,
                                            crop.width,
                                            crop.height,
                                            suffix
                                        ),
                                    )
                                    .clicked()
                                {
                                    self.selected_crop = Some(crop.id);
                                    self.asset_view = AssetView::Crop;
                                }
                            }
                        });
                    }

                    ui.separator();
                    ui.strong("Extracted sprites");
                    for sprite in &self.sprites {
                        let selected = self.selected_sprite == Some(sprite.id)
                            && self.asset_view == AssetView::Sprite;
                        if ui
                            .selectable_label(
                                selected,
                                format!("{}  [{} / {}]", sprite.name, sprite.kind.label(), sprite.category),
                            )
                            .clicked()
                        {
                            self.selected_sprite = Some(sprite.id);
                            self.asset_view = AssetView::Sprite;
                            self.sprite_zoom = 1.0;
                            self.sprite_pan = Vec2::ZERO;
                            self.sprite_overlay_drag = None;
                        }
                    }
                });
            });
    }

    fn asset_right_panel(&mut self, root_ui: &mut egui::Ui) {
        egui::Panel::right("asset_right")
            .resizable(true)
            .default_size(310.0)
            .show(root_ui, |ui| match self.asset_view {
                AssetView::Crop => self.crop_settings(ui),
                AssetView::Sprite => self.sprite_settings(ui),
            });
    }

    fn crop_settings(&mut self, ui: &mut egui::Ui) {
        ui.heading("Crop workspace");
        ui.horizontal(|ui| {
            ui.selectable_value(
                &mut self.crop_tool,
                CropTool::SelectMove,
                "Select / move / resize",
            );
            ui.selectable_value(&mut self.crop_tool, CropTool::Draw, "Draw crop");
        });
        ui.label(
            "Draw multiple crop rectangles. In Select mode, drag inside a crop to move it or drag its handles to resize it.",
        );
        ui.separator();

        let Some(source_index) = self.selected_source_index() else {
            ui.label("Import and select an image.");
            return;
        };
        let (source_w, source_h) = self.sources[source_index].image.dimensions();
        ui.label(format!("Source: {source_w} × {source_h} px"));
        if ui.button("Rotate 90° clockwise").clicked() {
            self.rotate_selected_source_clockwise();
            return;
        }

        let crop_index = self.selected_crop.and_then(|crop_id| {
            self.sources[source_index]
                .crops
                .iter()
                .position(|crop| crop.id == crop_id)
        });

        let mut extract = false;
        let mut copy = false;
        let mut delete = false;
        if let Some(crop_index) = crop_index {
            let has_sprite;
            {
                let crop = &mut self.sources[source_index].crops[crop_index];
                ui.separator();
                ui.strong("Selected crop");
                crop.x = crop.x.min(source_w.saturating_sub(crop.width));
                crop.y = crop.y.min(source_h.saturating_sub(crop.height));
                egui::Grid::new("crop_values").num_columns(3).show(ui, |ui| {
                    ui.label("X");
                    ui.add(
                        egui::Slider::new(
                            &mut crop.x,
                            0..=source_w.saturating_sub(crop.width),
                        )
                        .show_value(false),
                    );
                    ui.add(
                        egui::DragValue::new(&mut crop.x)
                            .range(0..=source_w.saturating_sub(crop.width)),
                    );
                    ui.end_row();
                    ui.label("Y");
                    ui.add(
                        egui::Slider::new(
                            &mut crop.y,
                            0..=source_h.saturating_sub(crop.height),
                        )
                        .show_value(false),
                    );
                    ui.add(
                        egui::DragValue::new(&mut crop.y)
                            .range(0..=source_h.saturating_sub(crop.height)),
                    );
                    ui.end_row();
                    ui.label("Width");
                    ui.add(
                        egui::Slider::new(
                            &mut crop.width,
                            1..=source_w.saturating_sub(crop.x).max(1),
                        )
                        .show_value(false),
                    );
                    ui.add(
                        egui::DragValue::new(&mut crop.width)
                            .range(1..=source_w.saturating_sub(crop.x).max(1)),
                    );
                    ui.end_row();
                    ui.label("Height");
                    ui.add(
                        egui::Slider::new(
                            &mut crop.height,
                            1..=source_h.saturating_sub(crop.y).max(1),
                        )
                        .show_value(false),
                    );
                    ui.add(
                        egui::DragValue::new(&mut crop.height)
                            .range(1..=source_h.saturating_sub(crop.y).max(1)),
                    );
                    ui.end_row();
                });
                crop.width = crop.width.min(source_w.saturating_sub(crop.x)).max(1);
                crop.height = crop.height.min(source_h.saturating_sub(crop.y)).max(1);
                has_sprite = crop.sprite_id.is_some();
            }

            ui.horizontal_wrapped(|ui| {
                extract = ui.button("Extract as sprite").clicked();
                copy = ui.button("Copy crop").clicked();
                delete = ui.button("Delete crop").clicked();
            });
            if has_sprite {
                ui.label(
                    "✓ This crop already has a sprite. Editing the crop does not change the extracted sprite.",
                );
            }
        } else {
            ui.label("Choose Draw crop, then drag over the image.");
        }

        ui.separator();
        let extract_all = ui.button("Extract all crops as sprites").clicked();

        if extract {
            self.extract_selected_crop();
        }
        if copy {
            self.copy_selected_crop();
        }
        if delete {
            self.delete_selected_crop();
        }
        if extract_all {
            self.extract_all_crops();
        }
    }

    fn sprite_settings(&mut self, ui: &mut egui::Ui) {
        ui.heading("Sprite settings");
        let Some(sprite_index) = self.selected_sprite_index() else {
            ui.label("Extract or select a sprite.");
            return;
        };

        ui.horizontal(|ui| {
            ui.add(
                egui::Slider::new(&mut self.sprite_zoom, 0.1..=12.0)
                    .logarithmic(true)
                    .text("Zoom"),
            );
            if ui.button("Fit").clicked() {
                self.sprite_zoom = 1.0;
                self.sprite_pan = Vec2::ZERO;
            }
        });
        ui.label("Mouse wheel: zoom. Middle mouse button: pan.");
        ui.separator();

        let mut delete = false;
        let mut run_smart_edge = false;
        let mut run_remove_color = false;
        let mut run_soften = false;
        let mut run_threshold = false;
        let mut reset_original = false;

        {
            let sprite = &mut self.sprites[sprite_index];
            ui.label("Name shown in Wonderdraft");
            ui.text_edit_singleline(&mut sprite.name);
            ui.label("PNG filename / metadata key");
            ui.text_edit_singleline(&mut sprite.file_stem);

            ComboBox::from_label("Asset type")
                .selected_text(sprite.kind.label())
                .show_ui(ui, |ui| {
                    for kind in AssetKind::ALL {
                        ui.selectable_value(&mut sprite.kind, kind, kind.label());
                    }
                });
            ui.label("Category folder (nested paths such as Cities/Capital are allowed)");
            ui.text_edit_singleline(&mut sprite.category);

            if sprite.kind.is_sprite() {
                ComboBox::from_label("Draw mode")
                    .selected_text(sprite.draw_mode.label())
                    .show_ui(ui, |ui| {
                        for mode in DrawMode::ALL {
                            ui.selectable_value(&mut sprite.draw_mode, mode, mode.label());
                        }
                    });
                let image_width = sprite.working.width().max(1) as i32;
                let image_height = sprite.working.height().max(1) as i32;
                let maximum_radius = image_width.max(image_height).saturating_mul(2);
                egui::Grid::new("symbol_metadata")
                    .num_columns(3)
                    .spacing(vec2(8.0, 6.0))
                    .show(ui, |ui| {
                        ui.label("Radius");
                        ui.add(
                            egui::Slider::new(&mut sprite.radius, 0..=maximum_radius)
                                .show_value(false),
                        );
                        ui.add(egui::DragValue::new(&mut sprite.radius).range(0..=100_000));
                        ui.end_row();

                        ui.label("Offset X");
                        ui.add(
                            egui::Slider::new(
                                &mut sprite.offset_x,
                                -image_width..=image_width,
                            )
                            .show_value(false),
                        );
                        ui.add(
                            egui::DragValue::new(&mut sprite.offset_x)
                                .range(-100_000..=100_000),
                        );
                        ui.end_row();

                        ui.label("Offset Y");
                        ui.add(
                            egui::Slider::new(
                                &mut sprite.offset_y,
                                -image_height..=image_height,
                            )
                            .show_value(false),
                        );
                        ui.add(
                            egui::DragValue::new(&mut sprite.offset_y)
                                .range(-100_000..=100_000),
                        );
                        ui.end_row();
                    });
                ui.horizontal(|ui| {
                    if ui.button("Center pivot").clicked() {
                        sprite.offset_x = 0;
                        sprite.offset_y = 0;
                    }
                    if ui.button("Bottom center").clicked() {
                        sprite.offset_x = 0;
                        sprite.offset_y = -(sprite.working.height() as i32 / 2);
                    }
                });
            }

            ui.separator();
            ui.strong("Transparency tools");
            ui.horizontal(|ui| {
                ui.selectable_value(&mut self.sprite_tool, SpriteTool::Erase, "Erase");
                ui.selectable_value(&mut self.sprite_tool, SpriteTool::Restore, "Restore");
                ui.selectable_value(&mut self.sprite_tool, SpriteTool::PickColor, "Pick color");
            });
            ui.add(egui::Slider::new(&mut self.brush_size, 1.0..=300.0).text("Brush diameter"));
            ui.add(egui::Slider::new(&mut self.tolerance, 0..=255).text("Color / alpha tolerance"));

            ui.horizontal(|ui| {
                ui.label("Picked color:");
                let swatch = Color32::from_rgb(
                    self.picked_color[0],
                    self.picked_color[1],
                    self.picked_color[2],
                );
                let (rect, _) = ui.allocate_exact_size(vec2(42.0, 22.0), Sense::hover());
                ui.painter().rect_filled(rect, 3.0, swatch);
                ui.label(format!(
                    "#{:02X}{:02X}{:02X}",
                    self.picked_color[0], self.picked_color[1], self.picked_color[2]
                ));
            });

            if ui.button("Remove picked color everywhere").clicked() {
                run_remove_color = true;
            }
            if ui.button("Smart edge-connected background removal").clicked() {
                run_smart_edge = true;
            }
            ui.add(egui::Slider::new(&mut self.alpha_blur_radius, 1..=12).text("Alpha soften radius"));
            if ui.button("Soften alpha edge").clicked() {
                run_soften = true;
            }
            if ui.button("Threshold alpha").clicked() {
                run_threshold = true;
            }

            ui.separator();
            ui.horizontal(|ui| {
                if ui.add_enabled(!sprite.undo.is_empty(), egui::Button::new("Undo")).clicked() {
                    sprite.undo();
                }
                if ui.add_enabled(!sprite.redo.is_empty(), egui::Button::new("Redo")).clicked() {
                    sprite.redo();
                }
                if ui.button("Reset original").clicked() {
                    reset_original = true;
                }
            });
            if ui.button("Delete sprite").clicked() {
                delete = true;
            }
        }

        if run_remove_color {
            let sprite = &mut self.sprites[sprite_index];
            sprite.push_undo();
            let count = remove_picked_color(&mut sprite.working, self.picked_color, self.tolerance);
            sprite.texture_dirty = true;
            self.status = format!("Removed {count} pixels matching the picked color.");
        }
        if run_smart_edge {
            let sprite = &mut self.sprites[sprite_index];
            sprite.push_undo();
            let count = smart_edge_remove(&mut sprite.working, self.tolerance);
            sprite.texture_dirty = true;
            self.status = format!("Removed {count} edge-connected background pixels.");
        }
        if run_soften {
            let sprite = &mut self.sprites[sprite_index];
            sprite.push_undo();
            soften_alpha(&mut sprite.working, self.alpha_blur_radius);
            sprite.texture_dirty = true;
            self.status = "Softened the alpha edge.".to_owned();
        }
        if run_threshold {
            let sprite = &mut self.sprites[sprite_index];
            sprite.push_undo();
            threshold_alpha(&mut sprite.working, self.tolerance);
            sprite.texture_dirty = true;
            self.status = format!("Applied alpha threshold {}.", self.tolerance);
        }
        if reset_original {
            let sprite = &mut self.sprites[sprite_index];
            sprite.push_undo();
            sprite.working = sprite.original.clone();
            sprite.texture_dirty = true;
            self.status = "Restored the complete original crop.".to_owned();
        }
        if delete {
            self.delete_selected_sprite();
        }
    }

    fn crop_canvas(&mut self, ui: &mut egui::Ui) {
        let available = vec2(ui.available_width().max(100.0), ui.available_height().max(100.0));
        let (canvas_rect, response) = ui.allocate_exact_size(available, Sense::click_and_drag());
        let painter = ui.painter_at(canvas_rect);
        painter.rect_filled(canvas_rect, 0.0, Color32::from_gray(24));

        let Some(source_index) = self.selected_source_index() else {
            painter.text(
                canvas_rect.center(),
                Align2::CENTER_CENTER,
                "Import an image, then draw multiple crop regions.",
                FontId::proportional(20.0),
                Color32::LIGHT_GRAY,
            );
            return;
        };

        let (texture_id, image_w, image_h) = {
            let source = &mut self.sources[source_index];
            if source.texture.is_none() {
                let source_id = source.id;
                let color_image = rgba_to_color_image(&source.image);
                source.texture = Some(ui.ctx().load_texture(
                    format!("source-{source_id}"),
                    color_image,
                    TextureOptions::LINEAR,
                ));
            }
            (
                source.texture.as_ref().unwrap().id(),
                source.image.width(),
                source.image.height(),
            )
        };
        let image_rect = fit_image_rect(canvas_rect.shrink(12.0), image_w, image_h);
        painter.image(
            texture_id,
            image_rect,
            Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)),
            Color32::WHITE,
        );

        for (index, crop) in self.sources[source_index].crops.iter().enumerate() {
            let rect = crop_to_screen_rect(crop, image_rect, image_w, image_h);
            let selected = self.selected_crop == Some(crop.id);
            let color = if selected {
                Color32::YELLOW
            } else if crop.sprite_id.is_some() {
                Color32::LIGHT_GREEN
            } else {
                Color32::LIGHT_BLUE
            };
            paint_shadowed_rect_stroke(
                &painter,
                rect,
                Stroke::new(if selected { 3.0 } else { 1.5 }, color),
            );
            painter.text(
                rect.min + vec2(4.0, 4.0),
                Align2::LEFT_TOP,
                format!("{}", index + 1),
                FontId::monospace(14.0),
                color,
            );
            if selected {
                for (_, handle_position) in crop_handle_positions(rect) {
                    let handle_rect =
                        Rect::from_center_size(handle_position, vec2(9.0, 9.0));
                    painter.rect_filled(handle_rect, 1.0, Color32::YELLOW);
                    painter.rect_stroke(
                        handle_rect,
                        1.0,
                        Stroke::new(1.0, Color32::BLACK),
                        StrokeKind::Inside,
                    );
                }
            }
        }

        if response.drag_started_by(PointerButton::Primary)
            || response.clicked_by(PointerButton::Primary)
        {
            if let Some(pointer) = response.interact_pointer_pos() {
                let hits_selected_handle = self.selected_crop.is_some_and(|selected_id| {
                    self.sources[source_index]
                        .crops
                        .iter()
                        .find(|crop| crop.id == selected_id)
                        .is_some_and(|crop| {
                            hit_crop_handle(
                                pointer,
                                crop_to_screen_rect(crop, image_rect, image_w, image_h),
                            )
                            .is_some()
                        })
                });
                let hits_crop = screen_to_image(pointer, image_rect, image_w, image_h)
                    .is_some_and(|point| {
                        self.sources[source_index]
                            .crops
                            .iter()
                            .any(|crop| point_in_crop(point, crop))
                    });
                self.crop_tool = if hits_selected_handle || hits_crop {
                    CropTool::SelectMove
                } else {
                    CropTool::Draw
                };
            }
        }

        if self.crop_tool == CropTool::Draw {
            self.handle_draw_crop(&response, image_rect, image_w, image_h, source_index);
            if let (Some(start), Some(current)) = (self.crop_drag_start, self.crop_drag_current) {
                let preview = image_points_to_screen_rect(start, current, image_rect, image_w, image_h);
                paint_shadowed_rect_stroke(
                    &painter,
                    preview,
                    Stroke::new(2.0, Color32::WHITE),
                );
            }
        } else {
            self.handle_select_move_crop(&response, image_rect, image_w, image_h, source_index);
        }
    }

    fn handle_draw_crop(
        &mut self,
        response: &egui::Response,
        image_rect: Rect,
        image_w: u32,
        image_h: u32,
        source_index: usize,
    ) {
        if response.drag_started_by(PointerButton::Primary) {
            if let Some(pos) = response.interact_pointer_pos() {
                if let Some(point) = screen_to_image(pos, image_rect, image_w, image_h) {
                    self.crop_drag_start = Some(point);
                    self.crop_drag_current = Some(point);
                }
            }
        }
        if response.dragged_by(PointerButton::Primary) {
            if let Some(pos) = response.interact_pointer_pos() {
                if let Some(point) = screen_to_image_clamped(pos, image_rect, image_w, image_h) {
                    self.crop_drag_current = Some(point);
                }
            }
        }
        if response.drag_stopped_by(PointerButton::Primary) {
            if let (Some(start), Some(end)) = (self.crop_drag_start.take(), self.crop_drag_current.take()) {
                let x0 = start.0.min(end.0).floor().max(0.0) as u32;
                let y0 = start.1.min(end.1).floor().max(0.0) as u32;
                let x1 = start.0.max(end.0).ceil().min(image_w as f32) as u32;
                let y1 = start.1.max(end.1).ceil().min(image_h as f32) as u32;
                if x1 > x0 && y1 > y0 {
                    let crop_id = self.alloc_id();
                    self.sources[source_index]
                        .crops
                        .push(CropRegion::new(crop_id, x0, y0, x1 - x0, y1 - y0));
                    self.selected_crop = Some(crop_id);
                    self.status = "Added a crop. Draw another or extract this one.".to_owned();
                }
            }
        }
    }

    fn handle_select_move_crop(
        &mut self,
        response: &egui::Response,
        image_rect: Rect,
        image_w: u32,
        image_h: u32,
        source_index: usize,
    ) {
        if let (Some(pointer), Some(selected_id)) = (response.hover_pos(), self.selected_crop) {
            if let Some(crop) = self.sources[source_index]
                .crops
                .iter()
                .find(|crop| crop.id == selected_id)
            {
                let crop_rect = crop_to_screen_rect(crop, image_rect, image_w, image_h);
                if let Some(handle) = hit_crop_handle(pointer, crop_rect) {
                    let cursor = match handle {
                        CropHandle::North | CropHandle::South => CursorIcon::ResizeVertical,
                        CropHandle::East | CropHandle::West => CursorIcon::ResizeHorizontal,
                        CropHandle::NorthWest | CropHandle::SouthEast => CursorIcon::ResizeNwSe,
                        CropHandle::NorthEast | CropHandle::SouthWest => CursorIcon::ResizeNeSw,
                    };
                    response.clone().on_hover_cursor(cursor);
                } else if point_in_crop(
                    screen_to_image_clamped(pointer, image_rect, image_w, image_h)
                        .unwrap_or((-1.0, -1.0)),
                    crop,
                ) {
                    response.clone().on_hover_cursor(CursorIcon::Grab);
                }
            }
        }

        if response.clicked_by(PointerButton::Primary) {
            if let Some(pointer) = response.interact_pointer_pos() {
                if let Some(point) = screen_to_image(pointer, image_rect, image_w, image_h) {
                    self.selected_crop = self.sources[source_index]
                        .crops
                        .iter()
                        .rev()
                        .find(|crop| point_in_crop(point, crop))
                        .map(|crop| crop.id);
                }
            }
        }

        if response.drag_started_by(PointerButton::Primary) {
            let Some(pointer) = response.interact_pointer_pos() else {
                return;
            };
            let Some(image_point) =
                screen_to_image_clamped(pointer, image_rect, image_w, image_h)
            else {
                return;
            };

            let mut drag = None;

            if let Some(selected_id) = self.selected_crop {
                if let Some(crop) = self.sources[source_index]
                    .crops
                    .iter()
                    .find(|crop| crop.id == selected_id)
                {
                    let crop_rect = crop_to_screen_rect(crop, image_rect, image_w, image_h);
                    if let Some(handle) = hit_crop_handle(pointer, crop_rect) {
                        drag = Some(CropDrag::Resize {
                            crop_id: crop.id,
                            handle,
                            start: image_point,
                            original: crop.clone(),
                        });
                    }
                }
            }

            if drag.is_none() {
                if let Some(crop) = self.sources[source_index]
                    .crops
                    .iter()
                    .rev()
                    .find(|crop| point_in_crop(image_point, crop))
                {
                    self.selected_crop = Some(crop.id);
                    drag = Some(CropDrag::Move {
                        crop_id: crop.id,
                        start: image_point,
                        original: crop.clone(),
                    });
                }
            }

            self.crop_drag = drag;
        }

        if response.dragged_by(PointerButton::Primary) {
            let Some(pointer) = response.interact_pointer_pos() else {
                return;
            };
            let Some(current) =
                screen_to_image_clamped(pointer, image_rect, image_w, image_h)
            else {
                return;
            };
            let Some(drag) = self.crop_drag.clone() else {
                return;
            };

            let crop_id = match &drag {
                CropDrag::Move { crop_id, .. } | CropDrag::Resize { crop_id, .. } => *crop_id,
            };
            let Some(crop) = self.sources[source_index]
                .crops
                .iter_mut()
                .find(|crop| crop.id == crop_id)
            else {
                return;
            };

            match drag {
                CropDrag::Move {
                    start, original, ..
                } => {
                    let dx = current.0 - start.0;
                    let dy = current.1 - start.1;
                    let max_x = image_w.saturating_sub(original.width) as f32;
                    let max_y = image_h.saturating_sub(original.height) as f32;
                    crop.x = (original.x as f32 + dx).round().clamp(0.0, max_x) as u32;
                    crop.y = (original.y as f32 + dy).round().clamp(0.0, max_y) as u32;
                }
                CropDrag::Resize {
                    handle,
                    start,
                    original,
                    ..
                } => {
                    resize_crop_from_drag(
                        crop,
                        &original,
                        handle,
                        current.0 - start.0,
                        current.1 - start.1,
                        image_w,
                        image_h,
                    );
                }
            }
        }

        if response.drag_stopped_by(PointerButton::Primary) {
            self.crop_drag = None;
        }
    }

    fn sprite_canvas(&mut self, ui: &mut egui::Ui) {
        let available = vec2(
            ui.available_width().max(100.0),
            ui.available_height().max(100.0),
        );
        let (canvas_rect, response) = ui.allocate_exact_size(available, Sense::click_and_drag());
        let painter = ui.painter_at(canvas_rect);
        painter.rect_filled(canvas_rect, 0.0, Color32::from_gray(28));

        let Some(sprite_index) = self.selected_sprite_index() else {
            painter.text(
                canvas_rect.center(),
                Align2::CENTER_CENTER,
                "Extract or select a sprite.",
                FontId::proportional(20.0),
                Color32::LIGHT_GRAY,
            );
            return;
        };

        let (texture_id, image_w, image_h, radius, offset_x, offset_y, kind) = {
            let sprite = &mut self.sprites[sprite_index];
            let color_image = rgba_to_color_image(&sprite.working);
            if sprite.texture.is_none() {
                sprite.texture = Some(ui.ctx().load_texture(
                    format!("sprite-{}", sprite.id),
                    color_image,
                    TextureOptions::NEAREST,
                ));
                sprite.texture_dirty = false;
            } else if sprite.texture_dirty {
                if let Some(texture) = &mut sprite.texture {
                    texture.set(color_image, TextureOptions::NEAREST);
                }
                sprite.texture_dirty = false;
            }
            (
                sprite.texture.as_ref().unwrap().id(),
                sprite.working.width(),
                sprite.working.height(),
                sprite.radius,
                sprite.offset_x,
                sprite.offset_y,
                sprite.kind,
            )
        };

        if response.hovered() {
            let scroll = ui.input(|input| input.smooth_scroll_delta.y);
            if scroll.abs() > 0.0 {
                let zoom_factor = (scroll * 0.0015).exp();
                self.sprite_zoom = (self.sprite_zoom * zoom_factor).clamp(0.1, 12.0);
            }

            let middle_down = ui.input(|input| {
                input.pointer.button_down(PointerButton::Middle)
            });
            if middle_down {
                let pointer_delta = ui.input(|input| input.pointer.delta());
                self.sprite_pan += pointer_delta;
            }
        }

        let fitted_rect = fit_image_rect(canvas_rect.shrink(20.0), image_w, image_h);
        let image_rect = Rect::from_center_size(
            canvas_rect.center() + self.sprite_pan,
            fitted_rect.size() * self.sprite_zoom,
        );

        paint_checkerboard(&painter, image_rect, 14.0);
        painter.image(
            texture_id,
            image_rect,
            Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)),
            Color32::WHITE,
        );
        painter.rect_stroke(
            image_rect,
            0.0,
            Stroke::new(1.0, Color32::GRAY),
            StrokeKind::Inside,
        );

        let overlay = if kind.is_sprite() {
            let scale_x = image_rect.width() / image_w.max(1) as f32;
            let scale_y = image_rect.height() / image_h.max(1) as f32;
            let scale = scale_x.min(scale_y);
            let pivot = pos2(
                image_rect.center().x + offset_x as f32 * scale_x,
                image_rect.center().y - offset_y as f32 * scale_y,
            );
            let radius_screen = radius.max(0) as f32 * scale;

            painter.circle_stroke(
                pivot,
                radius_screen,
                Stroke::new(2.0, Color32::from_rgb(255, 120, 80)),
            );
            painter.circle_filled(pivot, 8.0, Color32::from_rgb(255, 80, 60));
            painter.line_segment(
                [pivot - vec2(9.0, 0.0), pivot + vec2(9.0, 0.0)],
                Stroke::new(1.0, Color32::WHITE),
            );
            painter.line_segment(
                [pivot - vec2(0.0, 9.0), pivot + vec2(0.0, 9.0)],
                Stroke::new(1.0, Color32::WHITE),
            );

            Some(SpriteOverlay {
                pivot,
                radius_screen,
                scale,
            })
        } else {
            None
        };

        self.handle_sprite_interaction(
            &response,
            image_rect,
            image_w,
            image_h,
            sprite_index,
            overlay,
        );

        if response.hovered() {
            if let Some(pos) = response.hover_pos() {
                let overlay_hit = overlay.is_some_and(|overlay| {
                    pos.distance(overlay.pivot) <= SPRITE_PIVOT_HIT_RADIUS
                        || (pos.distance(overlay.pivot) - overlay.radius_screen).abs() <= 10.0
                });
                if image_rect.contains(pos)
                    && self.sprite_tool != SpriteTool::PickColor
                    && !overlay_hit
                {
                    let radius_points = (self.brush_size * 0.5)
                        * (image_rect.width() / image_w.max(1) as f32);
                    painter.circle_stroke(
                        pos,
                        radius_points,
                        Stroke::new(1.5, Color32::WHITE),
                    );
                }
            }
        }
    }

    fn handle_sprite_interaction(
        &mut self,
        response: &egui::Response,
        image_rect: Rect,
        image_w: u32,
        image_h: u32,
        sprite_index: usize,
        overlay: Option<SpriteOverlay>,
    ) {
        if let Some(overlay) = overlay {
            if let Some(pointer) = response.hover_pos() {
                let pivot_hit = pointer.distance(overlay.pivot) <= SPRITE_PIVOT_HIT_RADIUS;
                let radius_hit =
                    (pointer.distance(overlay.pivot) - overlay.radius_screen).abs() <= 10.0;
                if pivot_hit || radius_hit {
                    response.clone().on_hover_cursor(CursorIcon::Grab);
                }
            }

            if response.drag_started_by(PointerButton::Primary) {
                if let Some(pointer) = response.interact_pointer_pos() {
                    if pointer.distance(overlay.pivot) <= SPRITE_PIVOT_HIT_RADIUS {
                        self.sprite_overlay_drag = Some(SpriteOverlayDrag::Pivot);
                    } else if (pointer.distance(overlay.pivot) - overlay.radius_screen).abs()
                        <= 10.0
                    {
                        self.sprite_overlay_drag = Some(SpriteOverlayDrag::Radius);
                    }
                }
            }

            if let Some(drag_target) = self.sprite_overlay_drag {
                if response.dragged_by(PointerButton::Primary) {
                    if let Some(pointer) = response.interact_pointer_pos() {
                        match drag_target {
                            SpriteOverlayDrag::Pivot => {
                                if let Some((image_x, image_y)) = screen_to_image_clamped(
                                    pointer,
                                    image_rect,
                                    image_w,
                                    image_h,
                                ) {
                                    let sprite = &mut self.sprites[sprite_index];
                                    sprite.offset_x =
                                        (image_x - image_w as f32 / 2.0).round() as i32;
                                    sprite.offset_y =
                                        (image_h as f32 / 2.0 - image_y).round() as i32;
                                }
                            }
                            SpriteOverlayDrag::Radius => {
                                let radius = pointer.distance(overlay.pivot)
                                    / overlay.scale.max(0.0001);
                                self.sprites[sprite_index].radius =
                                    radius.round().max(0.0) as i32;
                            }
                        }
                    }
                }

                if response.drag_stopped_by(PointerButton::Primary) {
                    self.sprite_overlay_drag = None;
                }

                return;
            }
        }

        if self.sprite_tool == SpriteTool::PickColor {
            if response.clicked_by(PointerButton::Primary) {
                if let Some(pos) = response.interact_pointer_pos() {
                    if let Some((x, y)) = screen_to_pixel(pos, image_rect, image_w, image_h) {
                        let pixel = *self.sprites[sprite_index].working.get_pixel(x, y);
                        self.picked_color = [pixel[0], pixel[1], pixel[2]];
                        self.status = format!(
                            "Picked color #{:02X}{:02X}{:02X}.",
                            pixel[0], pixel[1], pixel[2]
                        );
                    }
                }
            }
            return;
        }

        let brush_mode = match self.sprite_tool {
            SpriteTool::Erase => BrushMode::Erase,
            SpriteTool::Restore => BrushMode::Restore,
            SpriteTool::PickColor => return,
        };
        let radius_pixels = self.brush_size * 0.5;

        if response.drag_started_by(PointerButton::Primary) {
            if let Some(pos) = response.interact_pointer_pos() {
                if let Some(point) = screen_to_image(pos, image_rect, image_w, image_h) {
                    self.sprites[sprite_index].push_undo();
                    self.brush_history_active = true;
                    let original = self.sprites[sprite_index].original.clone();
                    apply_brush_stamp(
                        &mut self.sprites[sprite_index].working,
                        &original,
                        point.0,
                        point.1,
                        radius_pixels,
                        brush_mode,
                    );
                    self.sprites[sprite_index].texture_dirty = true;
                    self.last_brush_point = Some(point);
                }
            }
        }
        if response.dragged_by(PointerButton::Primary) {
            if let Some(pos) = response.interact_pointer_pos() {
                if let Some(point) = screen_to_image_clamped(pos, image_rect, image_w, image_h) {
                    if !self.brush_history_active {
                        self.sprites[sprite_index].push_undo();
                        self.brush_history_active = true;
                    }
                    let from = self.last_brush_point.unwrap_or(point);
                    let original = self.sprites[sprite_index].original.clone();
                    apply_brush_line(
                        &mut self.sprites[sprite_index].working,
                        &original,
                        from,
                        point,
                        radius_pixels,
                        brush_mode,
                    );
                    self.sprites[sprite_index].texture_dirty = true;
                    self.last_brush_point = Some(point);
                }
            }
        }
        if response.drag_stopped_by(PointerButton::Primary) {
            self.last_brush_point = None;
            self.brush_history_active = false;
        }
        if response.clicked_by(PointerButton::Primary) {
            if let Some(pos) = response.interact_pointer_pos() {
                if let Some(point) = screen_to_image(pos, image_rect, image_w, image_h) {
                    self.sprites[sprite_index].push_undo();
                    let original = self.sprites[sprite_index].original.clone();
                    apply_brush_stamp(
                        &mut self.sprites[sprite_index].working,
                        &original,
                        point.0,
                        point.1,
                        radius_pixels,
                        brush_mode,
                    );
                    self.sprites[sprite_index].texture_dirty = true;
                }
            }
        }
    }

    fn themes_ui(&mut self, root_ui: &mut egui::Ui) {
        egui::Panel::left("theme_list")
            .resizable(true)
            .default_size(230.0)
            .show(root_ui, |ui| {
                ui.heading("Themes");
                if ui.button("New theme from template").clicked() {
                    self.themes.push(ThemeDraft::default());
                    self.selected_theme = self.themes.len() - 1;
                }
                ui.separator();
                for (index, theme) in self.themes.iter().enumerate() {
                    if ui
                        .selectable_label(index == self.selected_theme, &theme.file_stem)
                        .clicked()
                    {
                        self.selected_theme = index;
                    }
                }
            });

        egui::CentralPanel::default().show(root_ui, |ui| {
            if self.themes.is_empty() {
                self.themes.push(ThemeDraft::default());
                self.selected_theme = 0;
            }
            self.selected_theme = self.selected_theme.min(self.themes.len() - 1);
            let theme_index = self.selected_theme;
            let mut validate = false;
            let mut pretty = false;
            let mut duplicate = false;
            let mut delete = false;

            {
                let theme = &mut self.themes[theme_index];
                ui.heading("Wonderdraft theme file");
                ui.horizontal(|ui| {
                    ui.label("Filename:");
                    ui.text_edit_singleline(&mut theme.file_stem);
                    ui.label(".wonderdraft_theme");
                });
                ui.label(
                    "Edit every Wonderdraft theme property as JSON. Export validates and pretty-prints the file.",
                );
                ui.horizontal(|ui| {
                    validate = ui.button("Validate JSON").clicked();
                    pretty = ui.button("Pretty format").clicked();
                    duplicate = ui.button("Duplicate").clicked();
                    delete = ui.button("Delete").clicked();
                });
                ui.separator();
                egui::ScrollArea::vertical().show(ui, |ui| {
                    ui.add_sized(
                        ui.available_size(),
                        egui::TextEdit::multiline(&mut theme.json_text)
                            .code_editor()
                            .desired_rows(35)
                            .desired_width(f32::INFINITY),
                    );
                });
            }

            if validate {
                self.status = match serde_json::from_str::<serde_json::Value>(
                    &self.themes[theme_index].json_text,
                ) {
                    Ok(_) => "Theme JSON is valid.".to_owned(),
                    Err(error) => format!("Theme JSON error: {error}"),
                };
            }
            if pretty {
                match serde_json::from_str::<serde_json::Value>(
                    &self.themes[theme_index].json_text,
                ) {
                    Ok(value) => match serde_json::to_string_pretty(&value) {
                        Ok(text) => {
                            self.themes[theme_index].json_text = text;
                            self.status = "Formatted theme JSON.".to_owned();
                        }
                        Err(error) => self.status = format!("Formatting failed: {error}"),
                    },
                    Err(error) => self.status = format!("Theme JSON error: {error}"),
                }
            }
            if duplicate {
                let mut copy = self.themes[theme_index].clone();
                copy.file_stem.push_str("_copy");
                self.themes.push(copy);
                self.selected_theme = self.themes.len() - 1;
            }
            if delete && self.themes.len() > 1 {
                self.themes.remove(theme_index);
                self.selected_theme = self.selected_theme.min(self.themes.len() - 1);
            }
        });
    }
}

impl eframe::App for WonderdraftAssetStudio {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.handle_dropped_files(&ctx);
        self.handle_shortcuts(&ctx);
        self.top_bar(ui);
        self.status_bar(ui);
        match self.main_tab {
            MainTab::Assets => self.assets_ui(ui),
            MainTab::Themes => self.themes_ui(ui),
        }
        self.paint_drop_overlay(&ctx);
    }
}

fn rgba_to_color_image(image: &RgbaImage) -> ColorImage {
    ColorImage::from_rgba_unmultiplied(
        [image.width() as usize, image.height() as usize],
        image.as_raw(),
    )
}

fn fit_image_rect(container: Rect, width: u32, height: u32) -> Rect {
    let width = width.max(1) as f32;
    let height = height.max(1) as f32;
    let scale = (container.width() / width)
        .min(container.height() / height)
        .max(0.0001);
    Rect::from_center_size(container.center(), vec2(width * scale, height * scale))
}

fn paint_shadowed_rect_stroke(painter: &egui::Painter, rect: Rect, stroke: Stroke) {
    painter.rect_stroke(
        rect.translate(vec2(3.0, 3.0)),
        0.0,
        Stroke::new(stroke.width + 2.0, Color32::BLACK),
        StrokeKind::Inside,
    );
    painter.rect_stroke(rect, 0.0, stroke, StrokeKind::Inside);
}

fn rotate_crop_clockwise(crop: &mut CropRegion, old_image_height: u32) {
    let old_x = crop.x;
    crop.x = old_image_height.saturating_sub(crop.y.saturating_add(crop.height));
    crop.y = old_x;
    std::mem::swap(&mut crop.width, &mut crop.height);
}

fn adjust_crop_with_keys(
    crop: &mut CropRegion,
    source_w: u32,
    source_h: u32,
    resize: bool,
    left: bool,
    right: bool,
    up: bool,
    down: bool,
) {
    if resize {
        if left {
            crop.width = crop.width.saturating_sub(1).max(1);
        }
        if right {
            crop.width = crop
                .width
                .saturating_add(1)
                .min(source_w.saturating_sub(crop.x).max(1));
        }
        if up {
            crop.height = crop.height.saturating_sub(1).max(1);
        }
        if down {
            crop.height = crop
                .height
                .saturating_add(1)
                .min(source_h.saturating_sub(crop.y).max(1));
        }
    } else {
        if left {
            crop.x = crop.x.saturating_sub(1);
        }
        if right {
            crop.x = crop
                .x
                .saturating_add(1)
                .min(source_w.saturating_sub(crop.width));
        }
        if up {
            crop.y = crop.y.saturating_sub(1);
        }
        if down {
            crop.y = crop
                .y
                .saturating_add(1)
                .min(source_h.saturating_sub(crop.height));
        }
    }
}

fn paint_checkerboard(painter: &egui::Painter, rect: Rect, cell: f32) {
    let light = Color32::from_gray(180);
    let dark = Color32::from_gray(120);
    let columns = (rect.width() / cell).ceil() as usize;
    let rows = (rect.height() / cell).ceil() as usize;
    for y in 0..rows {
        for x in 0..columns {
            let min = pos2(rect.min.x + x as f32 * cell, rect.min.y + y as f32 * cell);
            let max = pos2((min.x + cell).min(rect.max.x), (min.y + cell).min(rect.max.y));
            painter.rect_filled(
                Rect::from_min_max(min, max),
                0.0,
                if (x + y) % 2 == 0 { light } else { dark },
            );
        }
    }
}

fn screen_to_image(pos: Pos2, rect: Rect, width: u32, height: u32) -> Option<(f32, f32)> {
    if !rect.contains(pos) {
        return None;
    }
    Some((
        ((pos.x - rect.min.x) / rect.width()) * width as f32,
        ((pos.y - rect.min.y) / rect.height()) * height as f32,
    ))
}

fn screen_to_image_clamped(
    pos: Pos2,
    rect: Rect,
    width: u32,
    height: u32,
) -> Option<(f32, f32)> {
    if rect.width() <= 0.0 || rect.height() <= 0.0 {
        return None;
    }
    Some((
        (((pos.x - rect.min.x) / rect.width()) * width as f32).clamp(0.0, width as f32),
        (((pos.y - rect.min.y) / rect.height()) * height as f32).clamp(0.0, height as f32),
    ))
}

fn screen_to_pixel(pos: Pos2, rect: Rect, width: u32, height: u32) -> Option<(u32, u32)> {
    screen_to_image(pos, rect, width, height).map(|(x, y)| {
        (
            x.floor().clamp(0.0, width.saturating_sub(1) as f32) as u32,
            y.floor().clamp(0.0, height.saturating_sub(1) as f32) as u32,
        )
    })
}

fn crop_to_screen_rect(crop: &CropRegion, rect: Rect, width: u32, height: u32) -> Rect {
    let sx = rect.width() / width.max(1) as f32;
    let sy = rect.height() / height.max(1) as f32;
    Rect::from_min_size(
        pos2(rect.min.x + crop.x as f32 * sx, rect.min.y + crop.y as f32 * sy),
        vec2(crop.width as f32 * sx, crop.height as f32 * sy),
    )
}

fn crop_handle_positions(rect: Rect) -> [(CropHandle, Pos2); 8] {
    [
        (CropHandle::NorthWest, rect.left_top()),
        (CropHandle::North, pos2(rect.center().x, rect.top())),
        (CropHandle::NorthEast, rect.right_top()),
        (CropHandle::East, pos2(rect.right(), rect.center().y)),
        (CropHandle::SouthEast, rect.right_bottom()),
        (CropHandle::South, pos2(rect.center().x, rect.bottom())),
        (CropHandle::SouthWest, rect.left_bottom()),
        (CropHandle::West, pos2(rect.left(), rect.center().y)),
    ]
}

fn hit_crop_handle(pointer: Pos2, rect: Rect) -> Option<CropHandle> {
    const HIT_RADIUS: f32 = 10.0;
    crop_handle_positions(rect)
        .into_iter()
        .find(|(_, position)| position.distance(pointer) <= HIT_RADIUS)
        .map(|(handle, _)| handle)
}

fn resize_crop_from_drag(
    crop: &mut CropRegion,
    original: &CropRegion,
    handle: CropHandle,
    dx: f32,
    dy: f32,
    image_width: u32,
    image_height: u32,
) {
    let original_left = original.x as f32;
    let original_top = original.y as f32;
    let original_right = (original.x + original.width) as f32;
    let original_bottom = (original.y + original.height) as f32;

    let mut left = original_left;
    let mut top = original_top;
    let mut right = original_right;
    let mut bottom = original_bottom;

    if matches!(
        handle,
        CropHandle::NorthWest | CropHandle::West | CropHandle::SouthWest
    ) {
        left = (original_left + dx).clamp(0.0, original_right - 1.0);
    }
    if matches!(
        handle,
        CropHandle::NorthEast | CropHandle::East | CropHandle::SouthEast
    ) {
        right = (original_right + dx).clamp(original_left + 1.0, image_width as f32);
    }
    if matches!(
        handle,
        CropHandle::NorthWest | CropHandle::North | CropHandle::NorthEast
    ) {
        top = (original_top + dy).clamp(0.0, original_bottom - 1.0);
    }
    if matches!(
        handle,
        CropHandle::SouthWest | CropHandle::South | CropHandle::SouthEast
    ) {
        bottom =
            (original_bottom + dy).clamp(original_top + 1.0, image_height as f32);
    }

    let x0 = left
        .round()
        .clamp(0.0, image_width.saturating_sub(1) as f32) as u32;
    let y0 = top
        .round()
        .clamp(0.0, image_height.saturating_sub(1) as f32) as u32;
    let x1 = right
        .round()
        .clamp((x0 + 1) as f32, image_width as f32) as u32;
    let y1 = bottom
        .round()
        .clamp((y0 + 1) as f32, image_height as f32) as u32;

    crop.x = x0;
    crop.y = y0;
    crop.width = x1 - x0;
    crop.height = y1 - y0;
}

fn image_points_to_screen_rect(
    a: (f32, f32),
    b: (f32, f32),
    rect: Rect,
    width: u32,
    height: u32,
) -> Rect {
    let sx = rect.width() / width.max(1) as f32;
    let sy = rect.height() / height.max(1) as f32;
    Rect::from_two_pos(
        pos2(rect.min.x + a.0 * sx, rect.min.y + a.1 * sy),
        pos2(rect.min.x + b.0 * sx, rect.min.y + b.1 * sy),
    )
}

fn point_in_crop(point: (f32, f32), crop: &CropRegion) -> bool {
    point.0 >= crop.x as f32
        && point.0 <= (crop.x + crop.width) as f32
        && point.1 >= crop.y as f32
        && point.1 <= (crop.y + crop.height) as f32
}

fn humanize(value: &str) -> String {
    let mut out = String::new();
    let mut previous_lower = false;
    for ch in value.chars() {
        if ch == '_' || ch == '-' {
            if !out.ends_with(' ') {
                out.push(' ');
            }
            previous_lower = false;
        } else {
            if ch.is_uppercase() && previous_lower {
                out.push(' ');
            }
            out.push(ch);
            previous_lower = ch.is_lowercase();
        }
    }
    out.trim().to_owned()
}

fn ensure_extension(mut path: PathBuf, extension: &str) -> PathBuf {
    if path.extension().and_then(|s| s.to_str()) != Some(extension) {
        path.set_extension(extension);
    }
    path
}


#[cfg(test)]
mod tests {
    use super::*;

    fn crop(x: u32, y: u32, width: u32, height: u32) -> CropRegion {
        CropRegion::new(1, x, y, width, height)
    }

    #[test]
    fn east_handle_increases_width() {
        let original = crop(10, 10, 20, 20);
        let mut changed = original.clone();
        resize_crop_from_drag(
            &mut changed,
            &original,
            CropHandle::East,
            10.0,
            0.0,
            100,
            100,
        );
        assert_eq!((changed.x, changed.y, changed.width, changed.height), (10, 10, 30, 20));
    }

    #[test]
    fn north_west_handle_moves_origin_and_shrinks_crop() {
        let original = crop(10, 10, 20, 20);
        let mut changed = original.clone();
        resize_crop_from_drag(
            &mut changed,
            &original,
            CropHandle::NorthWest,
            5.0,
            5.0,
            100,
            100,
        );
        assert_eq!((changed.x, changed.y, changed.width, changed.height), (15, 15, 15, 15));
    }

    #[test]
    fn resize_is_clamped_to_source_image() {
        let original = crop(10, 10, 20, 20);
        let mut changed = original.clone();
        resize_crop_from_drag(
            &mut changed,
            &original,
            CropHandle::West,
            -100.0,
            0.0,
            100,
            100,
        );
        assert_eq!((changed.x, changed.y, changed.width, changed.height), (0, 10, 30, 20));
    }

    #[test]
    fn clockwise_rotation_transforms_crop_coordinates() {
        let mut changed = crop(10, 20, 30, 40);
        rotate_crop_clockwise(&mut changed, 100);
        assert_eq!(
            (changed.x, changed.y, changed.width, changed.height),
            (40, 10, 40, 30)
        );
    }

    #[test]
    fn four_clockwise_rotations_restore_crop_coordinates() {
        let original = crop(10, 20, 30, 40);
        let mut changed = original.clone();
        let mut width = 120;
        let mut height = 100;
        for _ in 0..4 {
            rotate_crop_clockwise(&mut changed, height);
            std::mem::swap(&mut width, &mut height);
        }
        assert_eq!(
            (changed.x, changed.y, changed.width, changed.height),
            (original.x, original.y, original.width, original.height)
        );
    }

    #[test]
    fn keyboard_moves_crop_and_stays_inside_source() {
        let mut changed = crop(0, 0, 20, 20);
        adjust_crop_with_keys(&mut changed, 100, 100, false, true, false, true, false);
        assert_eq!((changed.x, changed.y), (0, 0));

        adjust_crop_with_keys(&mut changed, 100, 100, false, false, true, false, true);
        assert_eq!((changed.x, changed.y), (1, 1));
    }

    #[test]
    fn shifted_keyboard_resizes_crop() {
        let mut changed = crop(10, 10, 20, 20);
        adjust_crop_with_keys(&mut changed, 100, 100, true, true, false, true, false);
        assert_eq!((changed.width, changed.height), (19, 19));

        adjust_crop_with_keys(&mut changed, 100, 100, true, false, true, false, true);
        assert_eq!((changed.width, changed.height), (20, 20));
    }
}
