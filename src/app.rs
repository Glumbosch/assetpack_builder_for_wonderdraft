use std::{ops::RangeInclusive, path::PathBuf};

use eframe::egui::{
    self, pos2, vec2, Align2, Color32, ColorImage, ComboBox, CursorIcon, FontId, Key,
    PointerButton, Pos2, Rect, Sense, Stroke, StrokeKind, TextureOptions, Vec2,
};
use image::{imageops, RgbaImage};

use crate::{
    export::{export_pack, load_project, sanitize_file_stem, save_project, LoadedProject},
    image_ops::{
        apply_brush_line, apply_brush_stamp, crop_rgba, load_oriented_rgba, remove_picked_color,
        smart_edge_remove, soften_alpha, threshold_alpha, BrushMode,
    },
    model::{AssetKind, CropRegion, DrawMode, SourceImage, SpriteAsset, ThemeDraft},
    settings::{AppSettings, AppearanceMode},
    shortcuts::{ShortcutBinding, ShortcutSettings},
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FileDropTarget {
    Source,
    Sprite,
}

fn draw_mode_image(mode: DrawMode, size: f32) -> egui::Image<'static> {
    let icon = match mode {
        DrawMode::Normal => egui::include_image!("../app_assets/icons/rubber-stamp.svg"),
        DrawMode::SampleColor => egui::include_image!("../app_assets/icons/brush.svg"),
        DrawMode::CustomColors => egui::include_image!("../app_assets/icons/palette.svg"),
    };
    egui::Image::new(icon).fit_to_exact_size(vec2(size, size))
}

fn draw_mode_icon(ui: &mut egui::Ui, mode: DrawMode, size: f32) {
    let tint = ui.visuals().text_color();
    ui.add(draw_mode_image(mode, size).tint(tint));
}

fn photo_share_image(size: f32) -> egui::Image<'static> {
    egui::Image::new(egui::include_image!("../app_assets/icons/photo-share.svg"))
        .fit_to_exact_size(vec2(size, size))
}

#[derive(Debug, Clone, Copy)]
struct SpriteCropDrag {
    handle: CropHandle,
    start: Pos2,
    current: Pos2,
}

#[derive(Debug, Clone, Copy)]
struct SpriteOverlay {
    pivot: Pos2,
    radius_screen: f32,
    scale: f32,
}

const SPRITE_PIVOT_HIT_RADIUS: f32 = 24.0;
const WHEEL_DEBUG: bool = true;

#[derive(Debug, Clone)]
struct SourceDragPayload(u64);

pub struct AssetpackBuilderForWonderdraft {
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
    crop_zoom: f32,
    crop_pan: Vec2,
    square_selection: bool,
    keep_aspect_ratio: bool,

    sprite_overlay_drag: Option<SpriteOverlayDrag>,
    sprite_crop_drag: Option<SpriteCropDrag>,
    sprite_zoom: f32,
    sprite_pan: Vec2,

    erase_brush_size: f32,
    restore_brush_size: f32,
    tolerance: u8,
    alpha_blur_radius: u32,
    picked_color: [u8; 3],
    last_brush_point: Option<(f32, f32)>,
    brush_history_active: bool,

    project_path: Option<PathBuf>,
    install_root: Option<PathBuf>,
    settings: AppSettings,
    settings_draft: AppSettings,
    settings_open: bool,
    shortcut_capture: Option<&'static str>,
    editing_sprite_name: Option<u64>,
    source_drop_rect: Option<Rect>,
    sprite_drop_rect: Option<Rect>,
    file_drop_target: Option<FileDropTarget>,
    status: String,
}

impl Default for AssetpackBuilderForWonderdraft {
    fn default() -> Self {
        let settings = crate::settings::load();
        Self::from_settings(settings)
    }
}

impl AssetpackBuilderForWonderdraft {
    fn from_settings(settings: AppSettings) -> Self {
        Self {
            pack_name: settings.default_pack_name.clone(),
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
            crop_zoom: 1.0,
            crop_pan: Vec2::ZERO,
            square_selection: false,
            keep_aspect_ratio: false,
            sprite_overlay_drag: None,
            sprite_crop_drag: None,
            sprite_zoom: 1.0,
            sprite_pan: Vec2::ZERO,
            erase_brush_size: 24.0,
            restore_brush_size: 24.0,
            tolerance: 24,
            alpha_blur_radius: 2,
            picked_color: [255, 255, 255],
            last_brush_point: None,
            brush_history_active: false,
            project_path: None,
            install_root: settings.install_directory.clone(),
            settings_draft: settings.clone(),
            settings,
            settings_open: false,
            shortcut_capture: None,
            editing_sprite_name: None,
            source_drop_rect: None,
            sprite_drop_rect: None,
            file_drop_target: None,
            status: "Import images or drop them into a source or sprite area.".to_owned(),
        }
    }
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        egui_extras::install_image_loaders(&cc.egui_ctx);
        let app = Self::default();
        apply_appearance(&cc.egui_ctx, app.settings.appearance);
        cc.egui_ctx.all_styles_mut(|style| {
            style.spacing.button_padding = vec2(10.0, 10.0);
            style.spacing.interact_size.y = 36.0;
        });
        eprintln!(
            "[build-info] version={} build={} executable={}",
            env!("CARGO_PKG_VERSION"),
            env!("ASSETPACK_BUILDER_BUILD_ID"),
            std::env::current_exe()
                .map(|path| path.display().to_string())
                .unwrap_or_else(|error| format!("unavailable ({error})")),
        );
        app
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

    fn active_brush_size(&self, tool: SpriteTool) -> Option<f32> {
        match tool {
            SpriteTool::Erase => Some(self.erase_brush_size),
            SpriteTool::Restore => Some(self.restore_brush_size),
            SpriteTool::PickColor => None,
        }
    }

    fn adjust_active_brush_size(&mut self, amount: f32) {
        let brush_size = match self.sprite_tool {
            SpriteTool::Erase => &mut self.erase_brush_size,
            SpriteTool::Restore => &mut self.restore_brush_size,
            SpriteTool::PickColor => return,
        };
        *brush_size = (*brush_size + amount).clamp(1.0, 300.0);
    }

    fn sync_sprite_crop_to_source(&mut self, sprite_index: usize) {
        let source_id = self.sprites[sprite_index].source_id;
        let crop_id = self.sprites[sprite_index].crop_id;
        let mut bounds = self.sprites[sprite_index].crop_bounds.clone();
        bounds.id = crop_id;
        bounds.sprite_id = Some(self.sprites[sprite_index].id);
        self.sprites[sprite_index].crop_bounds = bounds.clone();

        if let Some(source_index) = self.source_index(source_id) {
            if let Some(crop) = self.sources[source_index]
                .crops
                .iter_mut()
                .find(|crop| crop.id == crop_id)
            {
                *crop = bounds;
            }
        }
    }

    fn import_dialog(&mut self) {
        if let Some(paths) = rfd::FileDialog::new()
            .add_filter(
                "Images",
                &["png", "jpg", "jpeg", "webp", "bmp", "tif", "tiff"],
            )
            .pick_files()
        {
            self.import_paths(paths);
        }
    }

    fn import_sprite_dialog(&mut self) {
        if let Some(paths) = rfd::FileDialog::new()
            .add_filter(
                "Images",
                &["png", "jpg", "jpeg", "webp", "bmp", "tif", "tiff"],
            )
            .pick_files()
        {
            self.import_direct_sprite_paths(paths);
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
                    self.crop_zoom = 1.0;
                    self.crop_pan = Vec2::ZERO;
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

    fn import_direct_sprite_paths<I>(&mut self, paths: I)
    where
        I: IntoIterator<Item = PathBuf>,
    {
        let previous_count = self.sources.len();
        self.import_paths(paths);
        let source_ids = self.sources[previous_count..]
            .iter()
            .map(|source| source.id)
            .collect::<Vec<_>>();
        let mut extracted = 0;
        for source_id in source_ids {
            if self.extract_source_as_whole_sprite(source_id) {
                extracted += 1;
            }
        }
        self.status = format!("Imported and extracted {extracted} whole image(s) as sprites.");
    }

    fn extract_source_as_whole_sprite(&mut self, source_id: u64) -> bool {
        let Some(source_index) = self.source_index(source_id) else {
            return false;
        };
        let crop_id = self.alloc_id();
        let (width, height) = self.sources[source_index].image.dimensions();
        self.sources[source_index]
            .crops
            .push(CropRegion::new(crop_id, 0, 0, width, height));
        self.selected_source = Some(source_id);
        self.selected_crop = Some(crop_id);
        let previous_count = self.sprites.len();
        self.extract_selected_crop();
        self.sprites.len() > previous_count
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
            rotate_crop_clockwise(&mut sprite.crop_bounds, old_height);
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
        let (has_hovered_files, dropped_files, pointer) = ctx.input(|input| {
            (
                !input.raw.hovered_files.is_empty(),
                input.raw.dropped_files.clone(),
                input.pointer.hover_pos(),
            )
        });

        if has_hovered_files {
            if let Some(pointer) = pointer {
                self.file_drop_target = self.drop_target_at(pointer);
            }
        }

        if dropped_files.is_empty() {
            return;
        }

        let paths = dropped_files
            .into_iter()
            .filter_map(|file| file.path)
            .collect::<Vec<_>>();

        let target = match pointer {
            Some(pointer) => self.drop_target_at(pointer),
            None => self.file_drop_target,
        };
        self.file_drop_target = None;
        if !paths.is_empty() {
            match target {
                Some(FileDropTarget::Source) => self.import_paths(paths),
                Some(FileDropTarget::Sprite) => self.import_direct_sprite_paths(paths),
                None => {
                    self.status =
                        "Drop images inside Source images or Extracted sprites.".to_owned();
                }
            }
        }
    }

    fn drop_target_at(&self, pointer: Pos2) -> Option<FileDropTarget> {
        if self
            .sprite_drop_rect
            .is_some_and(|rect| rect.contains(pointer))
        {
            Some(FileDropTarget::Sprite)
        } else if self
            .source_drop_rect
            .is_some_and(|rect| rect.contains(pointer))
        {
            Some(FileDropTarget::Source)
        } else {
            None
        }
    }

    fn debug_wheel_events(&self, ctx: &egui::Context) {
        if !WHEEL_DEBUG {
            return;
        }
        ctx.input(|input| {
            for event in &input.events {
                match event {
                    egui::Event::MouseWheel {
                        unit,
                        delta,
                        modifiers,
                        phase,
                    } => eprintln!(
                        "[wheel-debug][raw] tab={:?} view={:?} unit={unit:?} delta={delta:?} modifiers={modifiers:?} phase={phase:?} pointer={:?} smooth={:?} zoom_delta={:.4}",
                        self.main_tab,
                        self.asset_view,
                        input.pointer.hover_pos(),
                        input.smooth_scroll_delta,
                        input.zoom_delta(),
                    ),
                    egui::Event::Zoom(factor) => eprintln!(
                        "[wheel-debug][raw-zoom] tab={:?} view={:?} factor={factor:.4} pointer={:?}",
                        self.main_tab,
                        self.asset_view,
                        input.pointer.hover_pos(),
                    ),
                    _ => {}
                }
            }
        });
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
        let pointer = ctx.input(|input| input.pointer.hover_pos());
        let active_target = match pointer {
            Some(pointer) => self.drop_target_at(pointer),
            None => self.file_drop_target,
        };
        for (target, rect, label) in [
            (
                FileDropTarget::Source,
                self.source_drop_rect,
                "Drop as source image",
            ),
            (
                FileDropTarget::Sprite,
                self.sprite_drop_rect,
                "Drop as extracted sprite",
            ),
        ] {
            let Some(rect) = rect else {
                continue;
            };
            let active = active_target == Some(target);
            let color = if active {
                Color32::LIGHT_BLUE
            } else {
                Color32::GRAY
            };
            painter.rect_filled(
                rect,
                8.0,
                Color32::from_black_alpha(if active { 210 } else { 150 }),
            );
            painter.rect_stroke(
                rect,
                8.0,
                Stroke::new(if active { 4.0 } else { 2.0 }, color),
                StrokeKind::Inside,
            );
            painter.text(
                rect.center(),
                Align2::CENTER_CENTER,
                label,
                FontId::proportional(20.0),
                Color32::WHITE,
            );
        }
    }

    fn handle_shortcuts(&mut self, ctx: &egui::Context) {
        if self.settings_open || ctx.egui_wants_keyboard_input() {
            return;
        }

        let shortcuts = self.settings.shortcuts.clone();
        let pressed = |binding: ShortcutBinding| ctx.input(|input| binding.pressed(input));

        if pressed(shortcuts.save_project_as) {
            self.save_project_as_action();
        } else if pressed(shortcuts.save_project) {
            self.save_project_action();
        } else if pressed(shortcuts.open_project) {
            self.open_project_action();
        } else if pressed(shortcuts.new_project) {
            self.new_project();
        } else if pressed(shortcuts.assets_tab) {
            self.main_tab = MainTab::Assets;
        } else if pressed(shortcuts.themes_tab) {
            self.main_tab = MainTab::Themes;
        }

        if self.main_tab != MainTab::Assets {
            return;
        }

        if self.asset_view == AssetView::Crop {
            self.handle_crop_shortcuts(ctx, &shortcuts);
            return;
        }

        if pressed(shortcuts.sprite_delete) || pressed(shortcuts.sprite_delete_alternate) {
            self.delete_selected_sprite();
            return;
        }
        let pan_step = 24.0;
        if pressed(shortcuts.sprite_pan_left) {
            self.sprite_pan.x -= pan_step;
        }
        if pressed(shortcuts.sprite_pan_right) {
            self.sprite_pan.x += pan_step;
        }
        if pressed(shortcuts.sprite_pan_up) {
            self.sprite_pan.y -= pan_step;
        }
        if pressed(shortcuts.sprite_pan_down) {
            self.sprite_pan.y += pan_step;
        }
        if pressed(shortcuts.sprite_erase) {
            self.sprite_tool = SpriteTool::Erase;
            self.status = "Transparency tool: Erase.".to_owned();
        }
        if pressed(shortcuts.sprite_restore) {
            self.sprite_tool = SpriteTool::Restore;
            self.status = "Transparency tool: Restore.".to_owned();
        }
        if pressed(shortcuts.sprite_fit) {
            self.sprite_zoom = 1.0;
            self.sprite_pan = Vec2::ZERO;
        }
        if pressed(shortcuts.sprite_brush_smaller) {
            self.adjust_active_brush_size(-2.0);
        }
        if pressed(shortcuts.sprite_brush_larger) {
            self.adjust_active_brush_size(2.0);
        }

        let redo = pressed(shortcuts.sprite_redo);
        let undo = !redo && pressed(shortcuts.sprite_undo);

        let Some(sprite_index) = self.selected_sprite_index() else {
            return;
        };

        if redo {
            if self.sprites[sprite_index].redo() {
                self.sync_sprite_crop_to_source(sprite_index);
                self.status = "Redid sprite edit.".to_owned();
            }
        } else if undo && self.sprites[sprite_index].undo() {
            self.sync_sprite_crop_to_source(sprite_index);
            self.status = "Undid sprite edit.".to_owned();
        }
    }

    fn handle_crop_shortcuts(&mut self, ctx: &egui::Context, shortcuts: &ShortcutSettings) {
        let pressed = |binding: ShortcutBinding| ctx.input(|input| binding.pressed(input));
        if pressed(shortcuts.crop_delete) || pressed(shortcuts.crop_delete_alternate) {
            self.delete_selected_crop();
            return;
        }
        if pressed(shortcuts.crop_draw_tool) {
            self.crop_tool = CropTool::Draw;
        }
        if pressed(shortcuts.crop_select_tool) {
            self.crop_tool = CropTool::SelectMove;
        }
        if pressed(shortcuts.crop_fit) {
            self.crop_zoom = 1.0;
            self.crop_pan = Vec2::ZERO;
        }
        if pressed(shortcuts.crop_extract) {
            self.extract_selected_crop();
            return;
        }
        if pressed(shortcuts.crop_copy) {
            self.copy_selected_crop();
        }
        if pressed(shortcuts.crop_cancel) {
            self.crop_drag_start = None;
            self.crop_drag_current = None;
            self.crop_drag = None;
        }

        let pan_step = 24.0;
        if pressed(shortcuts.crop_pan_left) {
            self.crop_pan.x -= pan_step;
        }
        if pressed(shortcuts.crop_pan_right) {
            self.crop_pan.x += pan_step;
        }
        if pressed(shortcuts.crop_pan_up) {
            self.crop_pan.y -= pan_step;
        }
        if pressed(shortcuts.crop_pan_down) {
            self.crop_pan.y += pan_step;
        }

        let resize_left =
            pressed(shortcuts.crop_resize_left) || pressed(shortcuts.crop_resize_left_alternate);
        let resize_right =
            pressed(shortcuts.crop_resize_right) || pressed(shortcuts.crop_resize_right_alternate);
        let resize_up =
            pressed(shortcuts.crop_resize_up) || pressed(shortcuts.crop_resize_up_alternate);
        let resize_down =
            pressed(shortcuts.crop_resize_down) || pressed(shortcuts.crop_resize_down_alternate);
        let resize = resize_left || resize_right || resize_up || resize_down;
        let left = resize_left
            || pressed(shortcuts.crop_move_left)
            || pressed(shortcuts.crop_move_left_alternate);
        let right = resize_right
            || pressed(shortcuts.crop_move_right)
            || pressed(shortcuts.crop_move_right_alternate);
        let up = resize_up
            || pressed(shortcuts.crop_move_up)
            || pressed(shortcuts.crop_move_up_alternate);
        let down = resize_down
            || pressed(shortcuts.crop_move_down)
            || pressed(shortcuts.crop_move_down_alternate);

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

        let old_width = crop.width;
        let old_height = crop.height;
        adjust_crop_with_keys(crop, source_w, source_h, resize, left, right, up, down);
        if resize && self.square_selection {
            let side = if crop.width != old_width {
                crop.width
            } else {
                crop.height
            };
            set_crop_square_side(crop, side, source_w, source_h);
        } else if resize && self.keep_aspect_ratio {
            let aspect = old_width as f32 / old_height.max(1) as f32;
            if crop.width != old_width {
                set_crop_aspect_from_width(crop, crop.width, aspect, source_w, source_h);
            } else if crop.height != old_height {
                set_crop_aspect_from_height(crop, crop.height, aspect, source_w, source_h);
            }
        }
    }

    fn new_project(&mut self) {
        let settings = self.settings.clone();
        *self = Self::from_settings(settings);
        self.status = "Created a new project.".to_owned();
    }

    fn save_project_action(&mut self) {
        let path = match self.project_path.clone() {
            Some(path) => path,
            None => match rfd::FileDialog::new()
                .add_filter(
                    "Assetpack Builder for Wonderdraft project",
                    &["wdassetproj"],
                )
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
            .add_filter(
                "Assetpack Builder for Wonderdraft project",
                &["wdassetproj"],
            )
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
        self.crop_zoom = 1.0;
        self.crop_pan = Vec2::ZERO;
        self.sprite_zoom = 1.0;
        self.sprite_pan = Vec2::ZERO;
        self.sprite_overlay_drag = None;
        self.sprite_crop_drag = None;
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
        let root = if self.settings.export_without_asking {
            self.settings.export_directory.clone()
        } else {
            let mut dialog =
                rfd::FileDialog::new().set_title("Choose the Wonderdraft pack export folder");
            if let Some(directory) = &self.settings.export_directory {
                dialog = dialog.set_directory(directory);
            }
            dialog.pick_folder()
        };
        let Some(root) = root else {
            self.status = if self.settings.export_without_asking {
                "Set a default export folder in Settings, or disable export without asking."
                    .to_owned()
            } else {
                "Export cancelled.".to_owned()
            };
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

    fn install_asset_pack_action(&mut self) {
        let root = self
            .settings
            .install_directory
            .clone()
            .or_else(|| self.install_root.clone())
            .or_else(crate::settings::find_install_root)
            .or_else(|| {
                let default = crate::settings::default_wonderdraft_folder();
                let mut dialog = rfd::FileDialog::new().set_title(
                    "Choose the Wonderdraft folder containing config.ini, or its asset root",
                );
                if default.exists() {
                    dialog = dialog.set_directory(default);
                }
                dialog
                    .pick_folder()
                    .and_then(|path| crate::settings::install_root_from_selection(&path))
            });
        let Some(root) = root else {
            self.status = "Asset-pack installation cancelled.".to_owned();
            return;
        };

        match export_pack(&root, &self.pack_name, &self.sprites, &self.themes) {
            Ok(report) => {
                self.install_root = Some(root.clone());
                self.settings.install_directory = Some(root.clone());
                self.settings_draft = self.settings.clone();
                if let Err(error) = crate::settings::save(&self.settings) {
                    self.status = format!("Installed pack, but could not save settings: {error:#}");
                    return;
                }
                let mut status = format!(
                    "Installed {} PNG(s), {} .wonderdraft_symbols file(s), and {} theme(s) into {}.",
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
            Err(error) => self.status = format!("Installation failed: {error:#}"),
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
            crop_bounds: {
                let mut bounds = crop.clone();
                bounds.sprite_id = Some(sprite_id);
                bounds
            },
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
            let had_sprite = self.sources[source_index].crops[index].sprite_id.is_some();
            self.sources[source_index].crops.remove(index);
            self.selected_crop = self.sources[source_index].crops.first().map(|crop| crop.id);
            self.status = if had_sprite {
                "Deleted crop. Its already-extracted sprite remains independently editable."
                    .to_owned()
            } else {
                "Deleted crop.".to_owned()
            };
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
                ui.selectable_value(&mut self.main_tab, MainTab::Assets, "Assets");
                ui.selectable_value(&mut self.main_tab, MainTab::Themes, "Themes");
                ui.separator();
                ui.label("Pack name:");
                ui.add(egui::TextEdit::singleline(&mut self.pack_name).desired_width(180.0));
                ui.separator();
                if ui.button("Settings").clicked() {
                    self.settings_draft = self.settings.clone();
                    self.shortcut_capture = None;
                    self.settings_open = true;
                }
            });
        });
    }

    fn settings_window(&mut self, ctx: &egui::Context) {
        if !self.settings_open {
            return;
        }

        let captured_binding = self.shortcut_capture.and_then(|_| {
            ctx.input(|input| input.events.iter().find_map(ShortcutBinding::from_event))
        });

        let mut open = self.settings_open;
        let mut save_clicked = false;
        let mut cancel_clicked = false;
        egui::Window::new("Settings")
            .open(&mut open)
            .collapsible(false)
            .resizable(true)
            .default_width(650.0)
            .show(ctx, |ui| {
                egui::ScrollArea::vertical()
                    .max_height((ctx.content_rect().height() - 150.0).max(300.0))
                    .show(ui, |ui| {
                        ui.heading("General");
                        ComboBox::from_label("Appearance")
                            .selected_text(self.settings_draft.appearance.label())
                            .show_ui(ui, |ui| {
                                for appearance in AppearanceMode::ALL {
                                    ui.selectable_value(
                                        &mut self.settings_draft.appearance,
                                        appearance,
                                        appearance.label(),
                                    );
                                }
                        });
                        ui.label("App symbols follow the active text color.");
                        ui.add_space(8.0);
                        ui.strong("Build information");
                        ui.monospace(format!("Version: {}", env!("CARGO_PKG_VERSION")));
                        ui.monospace(format!(
                            "Build: {}",
                            env!("ASSETPACK_BUILDER_BUILD_ID")
                        ));
                        match std::env::current_exe() {
                            Ok(path) => {
                                ui.monospace(format!("Executable: {}", path.display()));
                            }
                            Err(error) => {
                                ui.label(format!("Executable path unavailable: {error}"));
                            }
                        }
                        ui.add_space(8.0);
                        ui.label("Wonderdraft asset-pack install directory");
                        path_setting_row(
                            ui,
                            &mut self.settings_draft.install_directory,
                            "Choose install directory",
                        );
                        ui.add_space(8.0);

                        ui.label("Default name for new packs");
                        ui.text_edit_singleline(&mut self.settings_draft.default_pack_name);
                        ui.add_space(8.0);

                        ui.label("Default Wonderdraft pack export directory");
                        path_setting_row(
                            ui,
                            &mut self.settings_draft.export_directory,
                            "Choose export directory",
                        );
                        ui.checkbox(
                            &mut self.settings_draft.export_without_asking,
                            "Do not ask for an export directory; export to the default folder",
                        );
                        if self.settings_draft.export_without_asking
                            && self.settings_draft.export_directory.is_none()
                        {
                            ui.colored_label(
                                Color32::YELLOW,
                                "Choose an export directory to use this option.",
                            );
                        }

                        ui.separator();
                        ui.heading("Keyboard shortcuts");
                        ui.label(
                            "Choose Change, then press a key or key combination. Modifier-only shortcuts such as Shift and Alt can also be assigned.",
                        );
                        let defaults = ShortcutSettings::default();
                        macro_rules! shortcut_row {
                            ($label:literal, $field:ident) => {
                                shortcut_setting_row(
                                    ui,
                                    ctx,
                                    $label,
                                    stringify!($field),
                                    &mut self.settings_draft.shortcuts.$field,
                                    defaults.$field,
                                    &mut self.shortcut_capture,
                                    captured_binding,
                                );
                            };
                        }

                        ui.add_space(6.0);
                        ui.strong("Application");
                        shortcut_row!("New project", new_project);
                        shortcut_row!("Open project", open_project);
                        shortcut_row!("Save project", save_project);
                        shortcut_row!("Save project as", save_project_as);
                        shortcut_row!("Show Assets tab", assets_tab);
                        shortcut_row!("Show Themes tab", themes_tab);

                        ui.add_space(8.0);
                        ui.strong("Crop workspace");
                        shortcut_row!("Delete selected crop", crop_delete);
                        shortcut_row!("Delete selected crop (alternate)", crop_delete_alternate);
                        shortcut_row!("Pan view left", crop_pan_left);
                        shortcut_row!("Pan view right", crop_pan_right);
                        shortcut_row!("Pan view up", crop_pan_up);
                        shortcut_row!("Pan view down", crop_pan_down);
                        shortcut_row!("Move crop left", crop_move_left);
                        shortcut_row!("Move crop right", crop_move_right);
                        shortcut_row!("Move crop up", crop_move_up);
                        shortcut_row!("Move crop down", crop_move_down);
                        shortcut_row!("Move crop left (alternate)", crop_move_left_alternate);
                        shortcut_row!("Move crop right (alternate)", crop_move_right_alternate);
                        shortcut_row!("Move crop up (alternate)", crop_move_up_alternate);
                        shortcut_row!("Move crop down (alternate)", crop_move_down_alternate);
                        shortcut_row!("Resize crop left", crop_resize_left);
                        shortcut_row!("Resize crop right", crop_resize_right);
                        shortcut_row!("Resize crop up", crop_resize_up);
                        shortcut_row!("Resize crop down", crop_resize_down);
                        shortcut_row!("Resize crop left (alternate)", crop_resize_left_alternate);
                        shortcut_row!("Resize crop right (alternate)", crop_resize_right_alternate);
                        shortcut_row!("Resize crop up (alternate)", crop_resize_up_alternate);
                        shortcut_row!("Resize crop down (alternate)", crop_resize_down_alternate);
                        shortcut_row!("Draw-crop tool", crop_draw_tool);
                        shortcut_row!("Select/move tool", crop_select_tool);
                        shortcut_row!("Fit image", crop_fit);
                        shortcut_row!("Extract selected crop", crop_extract);
                        shortcut_row!("Copy selected crop", crop_copy);
                        shortcut_row!("Cancel current crop interaction", crop_cancel);

                        ui.add_space(8.0);
                        ui.strong("Sprite settings workspace");
                        shortcut_row!("Delete selected sprite", sprite_delete);
                        shortcut_row!("Delete selected sprite (alternate)", sprite_delete_alternate);
                        shortcut_row!("Pan view left", sprite_pan_left);
                        shortcut_row!("Pan view right", sprite_pan_right);
                        shortcut_row!("Pan view up", sprite_pan_up);
                        shortcut_row!("Pan view down", sprite_pan_down);
                        shortcut_row!("Erase tool", sprite_erase);
                        shortcut_row!("Restore tool", sprite_restore);
                        shortcut_row!("Temporarily pick color", sprite_pick_color);
                        shortcut_row!("Temporarily pick color (alternate)", sprite_pick_color_alternate);
                        shortcut_row!("Temporarily crop sprite", sprite_crop_mode);
                        shortcut_row!("Hide pivot/radius overlays", sprite_hide_overlays);
                        shortcut_row!("Fit image", sprite_fit);
                        shortcut_row!("Decrease brush diameter", sprite_brush_smaller);
                        shortcut_row!("Increase brush diameter", sprite_brush_larger);
                        shortcut_row!(
                            "Erase diameter + mouse wheel modifier",
                            sprite_erase_wheel_adjust
                        );
                        shortcut_row!(
                            "Restore diameter + mouse wheel modifier",
                            sprite_restore_wheel_adjust
                        );
                        shortcut_row!(
                            "Pick tolerance + mouse wheel modifier",
                            sprite_pick_tolerance_wheel_adjust
                        );
                        shortcut_row!("Undo sprite edit", sprite_undo);
                        shortcut_row!("Redo sprite edit", sprite_redo);

                        ui.add_space(8.0);
                        if ui.button("Reset all keyboard shortcuts").clicked() {
                            self.settings_draft.shortcuts = ShortcutSettings::default();
                            self.shortcut_capture = None;
                        }
                    });

                ui.separator();
                ui.horizontal(|ui| {
                    save_clicked = ui.button("Save settings").clicked();
                    cancel_clicked = ui.button("Cancel").clicked();
                });
            });

        if save_clicked {
            if self.settings_draft.default_pack_name.trim().is_empty() {
                self.status = "The default pack name cannot be empty.".to_owned();
            } else if self.settings_draft.export_without_asking
                && self.settings_draft.export_directory.is_none()
            {
                self.status =
                    "Choose a default export folder before enabling automatic export.".to_owned();
            } else {
                self.settings_draft.default_pack_name =
                    self.settings_draft.default_pack_name.trim().to_owned();
                self.settings_draft.install_directory = self
                    .settings_draft
                    .install_directory
                    .as_deref()
                    .and_then(crate::settings::install_root_from_selection);
                match crate::settings::save(&self.settings_draft) {
                    Ok(()) => {
                        self.settings = self.settings_draft.clone();
                        apply_appearance(ctx, self.settings.appearance);
                        self.install_root = self.settings.install_directory.clone();
                        self.shortcut_capture = None;
                        self.settings_open = false;
                        self.status = "Saved settings.".to_owned();
                    }
                    Err(error) => self.status = format!("Could not save settings: {error:#}"),
                }
            }
        } else if cancel_clicked {
            self.settings_draft = self.settings.clone();
            self.shortcut_capture = None;
            self.settings_open = false;
        } else {
            self.settings_open = open;
            if !open {
                self.shortcut_capture = None;
            }
        }
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
                    let install_label = if self.install_root.is_some() {
                        "Install asset pack"
                    } else {
                        "Install asset pack…"
                    };
                    let install = ui.button(install_label);
                    let install = if let Some(root) = &self.install_root {
                        install.on_hover_text(format!("Install directly into {}", root.display()))
                    } else {
                        install.on_hover_text(
                            "Wonderdraft was not found automatically; choose its config or asset folder",
                        )
                    };
                    if install.clicked() {
                        self.install_asset_pack_action();
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
                ui.separator();

                egui::ScrollArea::vertical().show(ui, |ui| {
                    let mut source_clicked = None;
                    let source_section =
                        egui::Frame::group(ui.style())
                            .inner_margin(8.0)
                            .show(ui, |ui| {
                                ui.strong("Source images");
                                if ui.button("Import source images…").clicked() {
                                    self.import_dialog();
                                }
                                ui.label(
                                    "Drop files here, or drag a source down to Extracted sprites.",
                                );
                                for source in &self.sources {
                                    let selected = self.selected_source == Some(source.id)
                                        && self.asset_view == AssetView::Crop;
                                    let response = ui.add(
                                        egui::Button::new(source.display_name())
                                            .selected(selected)
                                            .frame(false)
                                            .sense(Sense::click_and_drag()),
                                    );
                                    response.dnd_set_drag_payload(SourceDragPayload(source.id));
                                    if response.clicked() {
                                        source_clicked = Some(source.id);
                                    }
                                }

                                if let Some(source_index) = self.selected_source_index() {
                                    ui.indent("crops", |ui| {
                                        for (index, crop) in
                                            self.sources[source_index].crops.iter().enumerate()
                                        {
                                            let selected = self.selected_crop == Some(crop.id)
                                                && self.asset_view == AssetView::Crop;
                                            let text = format!(
                                                "Crop {} — {}×{}",
                                                index + 1,
                                                crop.width,
                                                crop.height
                                            );
                                            let button = if crop.sprite_id.is_some() {
                                                egui::Button::image_and_text(
                                                    photo_share_image(18.0),
                                                    text,
                                                )
                                                .image_tint_follows_text_color(true)
                                            } else {
                                                egui::Button::new(text)
                                            }
                                            .selected(selected)
                                            .frame(false);
                                            let response = ui.add(button);
                                            let response = if crop.sprite_id.is_some() {
                                                response.on_hover_text(
                                                    "This crop has an extracted sprite",
                                                )
                                            } else {
                                                response
                                            };
                                            if response.clicked() {
                                                self.selected_crop = Some(crop.id);
                                                self.asset_view = AssetView::Crop;
                                            }
                                        }
                                    });
                                }
                            });
                    self.source_drop_rect = Some(source_section.response.rect);

                    if let Some(source_id) = source_clicked {
                        self.selected_source = Some(source_id);
                        self.selected_crop = self
                            .source_index(source_id)
                            .and_then(|index| self.sources[index].crops.first())
                            .map(|crop| crop.id);
                        self.asset_view = AssetView::Crop;
                        self.crop_zoom = 1.0;
                        self.crop_pan = Vec2::ZERO;
                    }

                    ui.separator();
                    let mut sprite_clicked = None;
                    let mut finish_edit = false;
                    let sprite_section =
                        egui::Frame::group(ui.style())
                            .inner_margin(8.0)
                            .show(ui, |ui| {
                                ui.strong("Extracted sprites");
                                if ui.button("Import whole images as sprites…").clicked() {
                                    self.import_sprite_dialog();
                                }
                                ui.label("Drop image files here to extract them without cropping.");
                                for sprite in &mut self.sprites {
                                    let selected = self.selected_sprite == Some(sprite.id)
                                        && self.asset_view == AssetView::Sprite;
                                    if self.editing_sprite_name == Some(sprite.id) {
                                        ui.horizontal(|ui| {
                                            let response = ui.add(
                                                egui::TextEdit::singleline(&mut sprite.name)
                                                    .desired_width(140.0),
                                            );
                                            if response.changed() {
                                                sprite.file_stem.clone_from(&sprite.name);
                                            }
                                            ui.label(format!(
                                                "[{} / {}]",
                                                sprite.kind.label(),
                                                sprite.category,
                                            ));
                                            draw_mode_icon(ui, sprite.draw_mode, 18.0);
                                            if response.lost_focus()
                                                || ui.input(|input| input.key_pressed(Key::Enter))
                                            {
                                                finish_edit = true;
                                            }
                                        });
                                    } else {
                                        let response = ui
                                            .add(
                                                egui::Button::image_and_text(
                                                    draw_mode_image(sprite.draw_mode, 18.0),
                                                    format!(
                                                        "{}  [{} / {}]",
                                                        sprite.name,
                                                        sprite.kind.label(),
                                                        sprite.category,
                                                    ),
                                                )
                                                .image_tint_follows_text_color(true)
                                                .selected(selected)
                                                .frame(false),
                                            )
                                            .on_hover_text(sprite.draw_mode.label());
                                        if response.double_clicked() {
                                            self.editing_sprite_name = Some(sprite.id);
                                        } else if response.clicked() {
                                            sprite_clicked = Some(sprite.id);
                                        }
                                    }
                                }
                            });
                    self.sprite_drop_rect = Some(sprite_section.response.rect);
                    let dropped_source = sprite_section
                        .response
                        .dnd_release_payload::<SourceDragPayload>()
                        .map(|payload| payload.0);

                    if finish_edit {
                        self.editing_sprite_name = None;
                    }
                    if let Some(sprite_id) = sprite_clicked {
                        self.selected_sprite = Some(sprite_id);
                        self.asset_view = AssetView::Sprite;
                        self.sprite_zoom = 1.0;
                        self.sprite_pan = Vec2::ZERO;
                        self.sprite_overlay_drag = None;
                        self.sprite_crop_drag = None;
                    }
                    if let Some(source_id) = dropped_source {
                        if self.extract_source_as_whole_sprite(source_id) {
                            self.status =
                                "Extracted the complete source image as a sprite.".to_owned();
                        }
                    }
                });
            });
    }

    fn asset_right_panel(&mut self, root_ui: &mut egui::Ui) {
        egui::Panel::right("asset_right")
            .resizable(true)
            .default_size(310.0)
            .show(root_ui, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| match self.asset_view {
                    AssetView::Crop => self.crop_settings(ui),
                    AssetView::Sprite => self.sprite_settings(ui),
                });
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
            "Draw multiple crop rectangles. In Select mode, drag inside a crop to move it or drag its handles to resize it. Wheel zoom stays anchored under the pointer; middle-drag pans.",
        );
        let square_changed = ui
            .checkbox(&mut self.square_selection, "Square selection")
            .changed();
        ui.checkbox(
            &mut self.keep_aspect_ratio,
            "Keep aspect ratio when resizing",
        );
        if self.square_selection {
            ui.label("Square selection takes precedence over the original aspect ratio.");
        }
        ui.separator();

        let Some(source_index) = self.selected_source_index() else {
            ui.label("Import and select an image.");
            return;
        };
        let (source_w, source_h) = self.sources[source_index].image.dimensions();
        if square_changed && self.square_selection {
            if let Some(crop_id) = self.selected_crop {
                if let Some(crop) = self.sources[source_index]
                    .crops
                    .iter_mut()
                    .find(|crop| crop.id == crop_id)
                {
                    reshape_crop_square(crop, source_w, source_h);
                }
            }
        }
        ui.label(format!("Source: {source_w} × {source_h} px"));
        ui.horizontal(|ui| {
            add_wheel_slider(
                ui,
                &mut self.crop_zoom,
                0.1..=12.0,
                SliderWheel::Multiplicative(1.1),
                Some("Zoom"),
                true,
                true,
            );
            if ui.button("Fit").clicked() {
                self.crop_zoom = 1.0;
                self.crop_pan = Vec2::ZERO;
            }
        });
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
                let old_width = crop.width;
                let old_height = crop.height;
                ui.separator();
                ui.strong("Selected crop");
                crop.x = crop.x.min(source_w.saturating_sub(crop.width));
                crop.y = crop.y.min(source_h.saturating_sub(crop.height));
                egui::Grid::new("crop_values")
                    .num_columns(3)
                    .show(ui, |ui| {
                        ui.label("X");
                        add_wheel_slider(
                            ui,
                            &mut crop.x,
                            0..=source_w.saturating_sub(crop.width),
                            SliderWheel::Linear(1.0),
                            None,
                            false,
                            false,
                        );
                        ui.add(
                            egui::DragValue::new(&mut crop.x)
                                .range(0..=source_w.saturating_sub(crop.width)),
                        );
                        ui.end_row();
                        ui.label("Y");
                        add_wheel_slider(
                            ui,
                            &mut crop.y,
                            0..=source_h.saturating_sub(crop.height),
                            SliderWheel::Linear(1.0),
                            None,
                            false,
                            false,
                        );
                        ui.add(
                            egui::DragValue::new(&mut crop.y)
                                .range(0..=source_h.saturating_sub(crop.height)),
                        );
                        ui.end_row();
                        ui.label("Width");
                        add_wheel_slider(
                            ui,
                            &mut crop.width,
                            1..=source_w.saturating_sub(crop.x).max(1),
                            SliderWheel::Linear(1.0),
                            None,
                            false,
                            false,
                        );
                        ui.add(
                            egui::DragValue::new(&mut crop.width)
                                .range(1..=source_w.saturating_sub(crop.x).max(1)),
                        );
                        ui.end_row();
                        ui.label("Height");
                        add_wheel_slider(
                            ui,
                            &mut crop.height,
                            1..=source_h.saturating_sub(crop.y).max(1),
                            SliderWheel::Linear(1.0),
                            None,
                            false,
                            false,
                        );
                        ui.add(
                            egui::DragValue::new(&mut crop.height)
                                .range(1..=source_h.saturating_sub(crop.y).max(1)),
                        );
                        ui.end_row();
                    });
                crop.width = crop.width.min(source_w.saturating_sub(crop.x)).max(1);
                crop.height = crop.height.min(source_h.saturating_sub(crop.y)).max(1);
                if self.square_selection {
                    let requested_side = if crop.width != old_width {
                        crop.width
                    } else if crop.height != old_height {
                        crop.height
                    } else {
                        crop.width.max(crop.height)
                    };
                    set_crop_square_side(crop, requested_side, source_w, source_h);
                } else if self.keep_aspect_ratio {
                    let aspect = old_width as f32 / old_height.max(1) as f32;
                    if crop.width != old_width {
                        set_crop_aspect_from_width(crop, crop.width, aspect, source_w, source_h);
                    } else if crop.height != old_height {
                        set_crop_aspect_from_height(crop, crop.height, aspect, source_w, source_h);
                    }
                }
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
            add_wheel_slider(
                ui,
                &mut self.sprite_zoom,
                0.1..=12.0,
                SliderWheel::Multiplicative(1.1),
                Some("Zoom"),
                true,
                true,
            );
            if ui.button("Fit").clicked() {
                self.sprite_zoom = 1.0;
                self.sprite_pan = Vec2::ZERO;
            }
        });
        ui.label(
            "Wheel zoom stays anchored under the pointer; middle-drag or Ctrl+Arrow pans. Hold Shift to hide/ignore pivot and radius handles. Hold Alt to crop the sprite.",
        );
        ui.label("Defaults: E erase, R restore, hold C or P to temporarily pick color.");
        ui.separator();

        let mut delete = false;
        let mut run_smart_edge = false;
        let mut run_remove_color = false;
        let mut run_soften = false;
        let mut run_threshold = false;
        let mut reset_original = false;
        let mut sprite_history_changed = false;

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
                ui.horizontal(|ui| {
                    ui.label("Draw mode");
                    draw_mode_icon(ui, sprite.draw_mode, 18.0);
                    ComboBox::from_id_salt("draw_mode")
                        .selected_text(sprite.draw_mode.label())
                        .show_ui(ui, |ui| {
                            for mode in DrawMode::ALL {
                                ui.horizontal(|ui| {
                                    draw_mode_icon(ui, mode, 18.0);
                                    ui.selectable_value(&mut sprite.draw_mode, mode, mode.label());
                                });
                            }
                        });
                });
                let image_width = sprite.working.width().max(1) as i32;
                let image_height = sprite.working.height().max(1) as i32;
                let maximum_radius = image_width.max(image_height).saturating_mul(2);
                egui::Grid::new("symbol_metadata")
                    .num_columns(3)
                    .spacing(vec2(8.0, 6.0))
                    .show(ui, |ui| {
                        ui.label("Radius");
                        add_wheel_slider(
                            ui,
                            &mut sprite.radius,
                            0..=maximum_radius,
                            SliderWheel::Linear(1.0),
                            None,
                            false,
                            false,
                        );
                        ui.add(egui::DragValue::new(&mut sprite.radius).range(0..=100_000));
                        ui.end_row();

                        ui.label("Offset X");
                        add_wheel_slider(
                            ui,
                            &mut sprite.offset_x,
                            -image_width..=image_width,
                            SliderWheel::Linear(1.0),
                            None,
                            false,
                            false,
                        );
                        ui.add(
                            egui::DragValue::new(&mut sprite.offset_x).range(-100_000..=100_000),
                        );
                        ui.end_row();

                        ui.label("Offset Y");
                        add_wheel_slider(
                            ui,
                            &mut sprite.offset_y,
                            -image_height..=image_height,
                            SliderWheel::Linear(1.0),
                            None,
                            false,
                            false,
                        );
                        ui.add(
                            egui::DragValue::new(&mut sprite.offset_y).range(-100_000..=100_000),
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
            match self.sprite_tool {
                SpriteTool::Erase => {
                    add_wheel_slider(
                        ui,
                        &mut self.erase_brush_size,
                        1.0..=300.0,
                        SliderWheel::Linear(2.0),
                        Some("Erase brush diameter"),
                        true,
                        false,
                    );
                }
                SpriteTool::Restore => {
                    add_wheel_slider(
                        ui,
                        &mut self.restore_brush_size,
                        1.0..=300.0,
                        SliderWheel::Linear(2.0),
                        Some("Restore brush diameter"),
                        true,
                        false,
                    );
                }
                SpriteTool::PickColor => {}
            }
            add_wheel_slider(
                ui,
                &mut self.tolerance,
                0..=255,
                SliderWheel::Linear(1.0),
                Some("Color / alpha tolerance"),
                true,
                false,
            );

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
            if ui
                .button("Smart edge-connected background removal")
                .clicked()
            {
                run_smart_edge = true;
            }
            add_wheel_slider(
                ui,
                &mut self.alpha_blur_radius,
                1..=12,
                SliderWheel::Linear(1.0),
                Some("Alpha soften radius"),
                true,
                false,
            );
            if ui.button("Soften alpha edge").clicked() {
                run_soften = true;
            }
            if ui.button("Threshold alpha").clicked() {
                run_threshold = true;
            }

            ui.separator();
            ui.horizontal(|ui| {
                if ui
                    .add_enabled(!sprite.undo.is_empty(), egui::Button::new("Undo"))
                    .clicked()
                {
                    sprite_history_changed = sprite.undo();
                }
                if ui
                    .add_enabled(!sprite.redo.is_empty(), egui::Button::new("Redo"))
                    .clicked()
                {
                    sprite_history_changed = sprite.redo();
                }
                if ui.button("Reset original").clicked() {
                    reset_original = true;
                }
            });
            if ui.button("Delete sprite").clicked() {
                delete = true;
            }
        }

        if sprite_history_changed {
            self.sync_sprite_crop_to_source(sprite_index);
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
        let available = vec2(
            ui.available_width().max(100.0),
            ui.available_height().max(100.0),
        );
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
        let fitted_rect = fit_image_rect(canvas_rect.shrink(12.0), image_w, image_h);
        let mut image_rect = Rect::from_center_size(
            canvas_rect.center() + self.crop_pan,
            fitted_rect.size() * self.crop_zoom,
        );
        let (scroll, gesture_zoom, pointer, middle_down, pointer_delta) = ui.input(|input| {
            (
                mouse_wheel_delta(input),
                input.zoom_delta(),
                input.pointer.hover_pos(),
                input.pointer.button_down(PointerButton::Middle),
                input.pointer.delta(),
            )
        });
        let pointer_over_canvas = pointer.is_some_and(|pointer| canvas_rect.contains(pointer));
        let pointer_over_image = pointer.is_some_and(|pointer| image_rect.contains(pointer));
        if scroll.abs() > f32::EPSILON || (gesture_zoom - 1.0).abs() > f32::EPSILON {
            let old_zoom = self.crop_zoom;
            let old_pan = self.crop_pan;
            if pointer_over_image {
                let factor = if scroll.abs() > f32::EPSILON {
                    (scroll * 0.0015).exp()
                } else {
                    gesture_zoom
                };
                zoom_at_pointer(
                    &mut self.crop_zoom,
                    &mut self.crop_pan,
                    pointer.expect("checked above"),
                    canvas_rect.center(),
                    factor,
                );
            }
            if WHEEL_DEBUG {
                eprintln!(
                    "[wheel-debug][crop-route] response_hovered={} pointer={pointer:?} over_canvas={pointer_over_canvas} over_image={pointer_over_image} wheel_points={scroll:.3} gesture_zoom={gesture_zoom:.4} zoom={old_zoom:.4}->{:.4} pan={old_pan:?}->{:?}",
                    response.hovered(),
                    self.crop_zoom,
                    self.crop_pan,
                );
            }
        }
        if middle_down && (response.hovered() || response.dragged()) {
            self.crop_pan += pointer_delta;
        }
        image_rect = Rect::from_center_size(
            canvas_rect.center() + self.crop_pan,
            fitted_rect.size() * self.crop_zoom,
        );
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
                    let handle_rect = Rect::from_center_size(handle_position, vec2(9.0, 9.0));
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

        if let (Some(pointer), Some(selected_id)) = (response.hover_pos(), self.selected_crop) {
            if let Some(crop) = self.sources[source_index]
                .crops
                .iter()
                .find(|crop| crop.id == selected_id)
            {
                let crop_rect = crop_to_screen_rect(crop, image_rect, image_w, image_h);
                if let Some(handle) = hit_crop_handle(pointer, crop_rect) {
                    response.clone().on_hover_cursor(crop_resize_cursor(handle));
                } else if crop_rect.contains(pointer) {
                    response.clone().on_hover_cursor(CursorIcon::Grab);
                }
            }
        }

        if response.double_clicked_by(PointerButton::Primary) {
            if let Some(pointer) = response.interact_pointer_pos() {
                if let Some(point) = screen_to_image(pointer, image_rect, image_w, image_h) {
                    if let Some(crop_id) = self.sources[source_index]
                        .crops
                        .iter()
                        .rev()
                        .find(|crop| point_in_crop(point, crop))
                        .map(|crop| crop.id)
                    {
                        self.selected_crop = Some(crop_id);
                        self.extract_selected_crop();
                        return;
                    }
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
                let hits_crop =
                    screen_to_image(pointer, image_rect, image_w, image_h).is_some_and(|point| {
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
                let current = if self.square_selection {
                    constrained_draw_endpoint(start, current, image_w, image_h, 1.0)
                } else {
                    current
                };
                let preview =
                    image_points_to_screen_rect(start, current, image_rect, image_w, image_h);
                paint_shadowed_rect_stroke(&painter, preview, Stroke::new(2.0, Color32::WHITE));
            }
        } else {
            self.handle_select_move_crop(&response, image_rect, image_w, image_h, source_index);
        }

        if ui.input(|input| input.pointer.button_down(PointerButton::Primary)) {
            match self.crop_drag.as_ref() {
                Some(CropDrag::Move { .. }) => ui.ctx().set_cursor_icon(CursorIcon::AllScroll),
                Some(CropDrag::Resize { handle, .. }) => {
                    ui.ctx().set_cursor_icon(crop_resize_cursor(*handle));
                }
                None => {}
            }
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
            if let (Some(start), Some(end)) =
                (self.crop_drag_start.take(), self.crop_drag_current.take())
            {
                let end = if self.square_selection {
                    constrained_draw_endpoint(start, end, image_w, image_h, 1.0)
                } else {
                    end
                };
                let x0 = start.0.min(end.0).floor().max(0.0) as u32;
                let y0 = start.1.min(end.1).floor().max(0.0) as u32;
                let x1 = start.0.max(end.0).ceil().min(image_w as f32) as u32;
                let y1 = start.1.max(end.1).ceil().min(image_h as f32) as u32;
                if x1 > x0 && y1 > y0 {
                    let crop_id = self.alloc_id();
                    let mut crop = CropRegion::new(crop_id, x0, y0, x1 - x0, y1 - y0);
                    if self.square_selection {
                        reshape_crop_square(&mut crop, image_w, image_h);
                    }
                    self.sources[source_index].crops.push(crop);
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
                    response.clone().on_hover_cursor(crop_resize_cursor(handle));
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
            let Some(image_point) = screen_to_image_clamped(pointer, image_rect, image_w, image_h)
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
            let Some(current) = screen_to_image_clamped(pointer, image_rect, image_w, image_h)
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
                        (image_w, image_h),
                        if self.square_selection {
                            Some(1.0)
                        } else if self.keep_aspect_ratio {
                            Some(original.width as f32 / original.height.max(1) as f32)
                        } else {
                            None
                        },
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

        let (
            texture_id,
            image_w,
            image_h,
            radius,
            offset_x,
            offset_y,
            kind,
            source_id,
            crop_bounds,
        ) = {
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
                sprite.source_id,
                sprite.crop_bounds.clone(),
            )
        };

        let (crop_mode_held, hide_overlays, pick_color_held) = ui.input(|input| {
            (
                self.settings.shortcuts.sprite_crop_mode.held(input),
                self.settings.shortcuts.sprite_hide_overlays.held(input),
                self.settings.shortcuts.sprite_pick_color.held(input)
                    || self
                        .settings
                        .shortcuts
                        .sprite_pick_color_alternate
                        .held(input),
            )
        });
        let crop_mode = crop_mode_held || self.sprite_crop_drag.is_some();
        let effective_tool = if pick_color_held {
            SpriteTool::PickColor
        } else {
            self.sprite_tool
        };
        let wheel_adjust_binding = match effective_tool {
            SpriteTool::Erase => self.settings.shortcuts.sprite_erase_wheel_adjust,
            SpriteTool::Restore => self.settings.shortcuts.sprite_restore_wheel_adjust,
            SpriteTool::PickColor => self.settings.shortcuts.sprite_pick_tolerance_wheel_adjust,
        };

        let fitted_rect = fit_image_rect(canvas_rect.shrink(20.0), image_w, image_h);
        let mut image_rect = Rect::from_center_size(
            canvas_rect.center() + self.sprite_pan,
            fitted_rect.size() * self.sprite_zoom,
        );
        let (zoom_scroll, adjust_scroll, gesture_zoom, pointer, middle_down, pointer_delta) = ui
            .input(|input| {
                let (zoom_scroll, adjust_scroll) =
                    partition_mouse_wheel_delta(input, wheel_adjust_binding, !crop_mode);
                (
                    zoom_scroll,
                    adjust_scroll,
                    input.zoom_delta(),
                    input.pointer.hover_pos(),
                    input.pointer.button_down(PointerButton::Middle),
                    input.pointer.delta(),
                )
            });
        let pointer_over_canvas = pointer.is_some_and(|pointer| canvas_rect.contains(pointer));
        let pointer_over_image = pointer.is_some_and(|pointer| image_rect.contains(pointer));
        let old_zoom = self.sprite_zoom;
        let old_pan = self.sprite_pan;
        let old_erase_size = self.erase_brush_size;
        let old_restore_size = self.restore_brush_size;
        let old_tolerance = self.tolerance;
        if pointer_over_image {
            if adjust_scroll.abs() > f32::EPSILON {
                let direction = adjust_scroll.signum();
                match effective_tool {
                    SpriteTool::Erase => {
                        self.erase_brush_size =
                            (self.erase_brush_size + direction * 2.0).clamp(1.0, 300.0);
                    }
                    SpriteTool::Restore => {
                        self.restore_brush_size =
                            (self.restore_brush_size + direction * 2.0).clamp(1.0, 300.0);
                    }
                    SpriteTool::PickColor => {
                        self.tolerance =
                            (self.tolerance as i16 + direction as i16).clamp(0, 255) as u8;
                    }
                }
            }
            let zoom_factor = if zoom_scroll.abs() > f32::EPSILON {
                Some((zoom_scroll * 0.0015).exp())
            } else if adjust_scroll.abs() <= f32::EPSILON
                && (gesture_zoom - 1.0).abs() > f32::EPSILON
            {
                Some(gesture_zoom)
            } else {
                None
            };
            if let Some(zoom_factor) = zoom_factor {
                zoom_at_pointer(
                    &mut self.sprite_zoom,
                    &mut self.sprite_pan,
                    pointer.expect("checked above"),
                    canvas_rect.center(),
                    zoom_factor,
                );
            }
        }
        if (zoom_scroll.abs() > f32::EPSILON
            || adjust_scroll.abs() > f32::EPSILON
            || (gesture_zoom - 1.0).abs() > f32::EPSILON)
            && WHEEL_DEBUG
        {
            eprintln!(
                "[wheel-debug][sprite-route] response_hovered={} pointer={pointer:?} over_canvas={pointer_over_canvas} over_image={pointer_over_image} crop_mode={crop_mode} tool={effective_tool:?} binding={wheel_adjust_binding:?} zoom_points={zoom_scroll:.3} adjust_points={adjust_scroll:.3} gesture_zoom={gesture_zoom:.4} zoom={old_zoom:.4}->{:.4} pan={old_pan:?}->{:?} erase={old_erase_size:.1}->{:.1} restore={old_restore_size:.1}->{:.1} tolerance={old_tolerance}->{}",
                response.hovered(),
                self.sprite_zoom,
                self.sprite_pan,
                self.erase_brush_size,
                self.restore_brush_size,
                self.tolerance,
            );
        }
        if middle_down && (response.hovered() || response.dragged()) {
            self.sprite_pan += pointer_delta;
        }
        image_rect = Rect::from_center_size(
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
        let crop_allowed_rect = self
            .source_index(source_id)
            .map(|source_index| {
                let source = &self.sources[source_index];
                let scale_x = image_rect.width() / image_w.max(1) as f32;
                let scale_y = image_rect.height() / image_h.max(1) as f32;
                Rect::from_min_max(
                    pos2(
                        image_rect.left() - crop_bounds.x as f32 * scale_x,
                        image_rect.top() - crop_bounds.y as f32 * scale_y,
                    ),
                    pos2(
                        image_rect.right()
                            + source
                                .image
                                .width()
                                .saturating_sub(crop_bounds.x.saturating_add(image_w))
                                as f32
                                * scale_x,
                        image_rect.bottom()
                            + source
                                .image
                                .height()
                                .saturating_sub(crop_bounds.y.saturating_add(image_h))
                                as f32
                                * scale_y,
                    ),
                )
            })
            .unwrap_or(image_rect);

        let overlay = if kind.is_sprite() && !hide_overlays && !crop_mode {
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

        if crop_mode {
            paint_sprite_crop_handles(&painter, image_rect);
            if let Some(drag) = self.sprite_crop_drag {
                let preview = sprite_crop_preview_rect(image_rect, crop_allowed_rect, drag);
                painter.rect_filled(
                    image_rect,
                    0.0,
                    Color32::from_rgba_unmultiplied(0, 0, 0, 80),
                );
                paint_checkerboard(&painter, preview, 14.0);
                let existing_pixels = preview.intersect(image_rect);
                if existing_pixels.is_positive() {
                    painter.image(
                        texture_id,
                        existing_pixels,
                        screen_rect_to_uv(existing_pixels, image_rect),
                        Color32::WHITE,
                    );
                }
                paint_shadowed_rect_stroke(&painter, preview, Stroke::new(2.0, Color32::YELLOW));
            }
            if self.handle_sprite_crop_interaction(
                &response,
                image_rect,
                image_w,
                image_h,
                sprite_index,
                crop_allowed_rect,
            ) {
                return;
            }
        } else {
            self.handle_sprite_interaction(
                &response,
                image_rect,
                image_w,
                image_h,
                sprite_index,
                overlay,
                effective_tool,
            );
        }

        if response.hovered() {
            if let Some(pos) = response.hover_pos() {
                let overlay_hit = overlay.is_some_and(|overlay| {
                    pos.distance(overlay.pivot) <= SPRITE_PIVOT_HIT_RADIUS
                        || (pos.distance(overlay.pivot) - overlay.radius_screen).abs() <= 10.0
                });
                if image_rect.contains(pos)
                    && effective_tool != SpriteTool::PickColor
                    && !crop_mode
                    && !overlay_hit
                {
                    if let Some(brush_size) = self.active_brush_size(effective_tool) {
                        let radius_points =
                            (brush_size * 0.5) * (image_rect.width() / image_w.max(1) as f32);
                        painter.circle_stroke(pos, radius_points, Stroke::new(1.5, Color32::WHITE));
                    }
                }
            }
        }
    }

    fn handle_sprite_crop_interaction(
        &mut self,
        response: &egui::Response,
        image_rect: Rect,
        image_w: u32,
        image_h: u32,
        sprite_index: usize,
        crop_allowed_rect: Rect,
    ) -> bool {
        if let Some(pointer) = response.hover_pos() {
            if let Some(handle) = hit_crop_handle_with_radius(pointer, image_rect, 20.0) {
                response.clone().on_hover_cursor(crop_resize_cursor(handle));
            }
        }

        if response.drag_started_by(PointerButton::Primary) {
            if let Some(pointer) = response.interact_pointer_pos() {
                if let Some(handle) = hit_crop_handle_with_radius(pointer, image_rect, 20.0) {
                    self.sprite_crop_drag = Some(SpriteCropDrag {
                        handle,
                        start: pointer,
                        current: pointer,
                    });
                }
            }
        }

        if response.dragged_by(PointerButton::Primary) {
            if let (Some(pointer), Some(drag)) = (
                response.interact_pointer_pos(),
                self.sprite_crop_drag.as_mut(),
            ) {
                drag.current = pointer;
            }
        }

        if response.drag_stopped_by(PointerButton::Primary) {
            let Some(drag) = self.sprite_crop_drag.take() else {
                return false;
            };
            let preview = sprite_crop_preview_rect(image_rect, crop_allowed_rect, drag);
            let scale_x = image_rect.width() / image_w.max(1) as f32;
            let scale_y = image_rect.height() / image_h.max(1) as f32;
            let local_x0 = ((preview.left() - image_rect.left()) / scale_x).floor() as i32;
            let local_y0 = ((preview.top() - image_rect.top()) / scale_y).floor() as i32;
            let local_x1 = ((preview.right() - image_rect.left()) / scale_x).ceil() as i32;
            let local_y1 = ((preview.bottom() - image_rect.top()) / scale_y).ceil() as i32;
            if local_x0 == 0
                && local_y0 == 0
                && local_x1 == image_w as i32
                && local_y1 == image_h as i32
            {
                return false;
            }

            let source_id = self.sprites[sprite_index].source_id;
            let old_bounds = self.sprites[sprite_index].crop_bounds.clone();
            let old_working = self.sprites[sprite_index].working.clone();
            let Some(source_index) = self.source_index(source_id) else {
                return false;
            };
            let source = &self.sources[source_index].image;
            let source_w = source.width() as i64;
            let source_h = source.height() as i64;
            let global_x0 =
                (old_bounds.x as i64 + local_x0 as i64).clamp(0, source_w.saturating_sub(1));
            let global_y0 =
                (old_bounds.y as i64 + local_y0 as i64).clamp(0, source_h.saturating_sub(1));
            let global_x1 = (old_bounds.x as i64 + local_x1 as i64).clamp(global_x0 + 1, source_w);
            let global_y1 = (old_bounds.y as i64 + local_y1 as i64).clamp(global_y0 + 1, source_h);
            let mut new_bounds = old_bounds.clone();
            new_bounds.x = global_x0 as u32;
            new_bounds.y = global_y0 as u32;
            new_bounds.width = (global_x1 - global_x0) as u32;
            new_bounds.height = (global_y1 - global_y0) as u32;
            let (new_original, new_working) =
                reframe_sprite_from_source(source, &old_working, &old_bounds, &new_bounds);
            let actual_local_x0 = new_bounds.x as i64 - old_bounds.x as i64;
            let actual_local_y0 = new_bounds.y as i64 - old_bounds.y as i64;

            {
                let sprite = &mut self.sprites[sprite_index];
                sprite.push_undo();
                sprite.crop_bounds = new_bounds;
                sprite.original = new_original;
                sprite.working = new_working;
                adjust_offsets_after_sprite_reframe(
                    &mut sprite.offset_x,
                    &mut sprite.offset_y,
                    image_w,
                    image_h,
                    actual_local_x0 as i32,
                    actual_local_y0 as i32,
                    sprite.crop_bounds.width,
                    sprite.crop_bounds.height,
                );
                sprite.texture_dirty = true;
            }
            self.sync_sprite_crop_to_source(sprite_index);
            self.status = format!(
                "Cropped sprite to {} × {} px. Undo restores the previous bounds.",
                self.sprites[sprite_index].crop_bounds.width,
                self.sprites[sprite_index].crop_bounds.height,
            );
            return true;
        }

        false
    }

    #[allow(clippy::too_many_arguments)]
    fn handle_sprite_interaction(
        &mut self,
        response: &egui::Response,
        image_rect: Rect,
        image_w: u32,
        image_h: u32,
        sprite_index: usize,
        overlay: Option<SpriteOverlay>,
        effective_tool: SpriteTool,
    ) {
        if let Some(overlay) = overlay {
            if let Some(pointer) = response.hover_pos() {
                let pivot_hit = pointer.distance(overlay.pivot) <= SPRITE_PIVOT_HIT_RADIUS;
                let radius_hit =
                    (pointer.distance(overlay.pivot) - overlay.radius_screen).abs() <= 10.0;
                if pivot_hit {
                    response.clone().on_hover_cursor(CursorIcon::Grab);
                } else if radius_hit {
                    response
                        .clone()
                        .on_hover_cursor(radius_resize_cursor(overlay.pivot, pointer));
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
                match drag_target {
                    SpriteOverlayDrag::Pivot => {
                        response.clone().on_hover_cursor(CursorIcon::AllScroll);
                        response.ctx.set_cursor_icon(CursorIcon::AllScroll);
                    }
                    SpriteOverlayDrag::Radius => {
                        if let Some(pointer) = response.interact_pointer_pos() {
                            let cursor = radius_resize_cursor(overlay.pivot, pointer);
                            response.clone().on_hover_cursor(cursor);
                            response.ctx.set_cursor_icon(cursor);
                        }
                    }
                }
                if response.dragged_by(PointerButton::Primary) {
                    if let Some(pointer) = response.interact_pointer_pos() {
                        match drag_target {
                            SpriteOverlayDrag::Pivot => {
                                if let Some((image_x, image_y)) =
                                    screen_to_image_clamped(pointer, image_rect, image_w, image_h)
                                {
                                    let sprite = &mut self.sprites[sprite_index];
                                    sprite.offset_x =
                                        (image_x - image_w as f32 / 2.0).round() as i32;
                                    sprite.offset_y =
                                        (image_h as f32 / 2.0 - image_y).round() as i32;
                                }
                            }
                            SpriteOverlayDrag::Radius => {
                                let radius =
                                    pointer.distance(overlay.pivot) / overlay.scale.max(0.0001);
                                self.sprites[sprite_index].radius = radius.round().max(0.0) as i32;
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

        if effective_tool == SpriteTool::PickColor {
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

        let brush_mode = match effective_tool {
            SpriteTool::Erase => BrushMode::Erase,
            SpriteTool::Restore => BrushMode::Restore,
            SpriteTool::PickColor => return,
        };
        let radius_pixels = self.active_brush_size(effective_tool).unwrap_or(1.0) * 0.5;

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

impl eframe::App for AssetpackBuilderForWonderdraft {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.debug_wheel_events(&ctx);
        self.handle_dropped_files(&ctx);
        self.handle_shortcuts(&ctx);
        self.top_bar(ui);
        self.status_bar(ui);
        match self.main_tab {
            MainTab::Assets => self.assets_ui(ui),
            MainTab::Themes => self.themes_ui(ui),
        }
        self.settings_window(&ctx);
        self.paint_drop_overlay(&ctx);
    }
}

#[derive(Debug, Clone, Copy)]
enum SliderWheel {
    Linear(f64),
    Multiplicative(f64),
}

fn apply_appearance(ctx: &egui::Context, appearance: AppearanceMode) {
    let visuals = match appearance {
        AppearanceMode::Dark => egui::Visuals::dark(),
        AppearanceMode::Light => egui::Visuals::light(),
    };
    ctx.set_visuals(visuals);
}

fn mouse_wheel_delta(input: &egui::InputState) -> f32 {
    input
        .events
        .iter()
        .filter_map(|event| {
            if let egui::Event::MouseWheel { unit, delta, .. } = event {
                Some(mouse_wheel_delta_in_points(*unit, *delta))
            } else {
                None
            }
        })
        .sum()
}

fn mouse_wheel_delta_in_points(unit: egui::MouseWheelUnit, delta: Vec2) -> f32 {
    const LINE_POINTS: f32 = 40.0;
    const PAGE_POINTS: f32 = 800.0;
    match unit {
        egui::MouseWheelUnit::Point => delta.y,
        egui::MouseWheelUnit::Line => delta.y * LINE_POINTS,
        egui::MouseWheelUnit::Page => delta.y * PAGE_POINTS,
    }
}

fn partition_mouse_wheel_delta(
    input: &egui::InputState,
    adjustment_binding: ShortcutBinding,
    adjustment_enabled: bool,
) -> (f32, f32) {
    let mut zoom_delta = 0.0;
    let mut adjustment_delta = 0.0;
    for event in &input.events {
        if let egui::Event::MouseWheel {
            unit,
            delta,
            modifiers,
            ..
        } = event
        {
            let delta = mouse_wheel_delta_in_points(*unit, *delta);
            if adjustment_enabled && adjustment_binding.held_during_wheel(input, *modifiers) {
                adjustment_delta += delta;
            } else {
                zoom_delta += delta;
            }
        }
    }
    (zoom_delta, adjustment_delta)
}

fn add_wheel_slider<T: egui::emath::Numeric>(
    ui: &mut egui::Ui,
    value: &mut T,
    range: RangeInclusive<T>,
    wheel: SliderWheel,
    text: Option<&str>,
    show_value: bool,
    logarithmic: bool,
) -> egui::Response {
    let min = range.start().to_f64().min(range.end().to_f64());
    let max = range.start().to_f64().max(range.end().to_f64());
    let keyboard_step = match wheel {
        SliderWheel::Linear(step) => step,
        SliderWheel::Multiplicative(_) => ((max - min) / 100.0).max(0.01),
    };
    let mut slider = egui::Slider::new(value, range).step_by(keyboard_step);
    if let Some(text) = text {
        slider = slider.text(text);
    }
    if !show_value {
        slider = slider.show_value(false);
    }
    if logarithmic {
        slider = slider.logarithmic(true);
    }
    let mut response = ui.add(slider);
    if response.clicked() || response.drag_started() {
        response.request_focus();
    }

    if response.hovered() {
        let scroll = ui.input(mouse_wheel_delta);
        if scroll.abs() > f32::EPSILON {
            let current = value.to_f64();
            let next = match wheel {
                SliderWheel::Linear(step) => current + step.copysign(scroll as f64),
                SliderWheel::Multiplicative(factor) if scroll > 0.0 => current * factor,
                SliderWheel::Multiplicative(factor) => current / factor,
            }
            .clamp(min, max);
            *value = T::from_f64(next);
            response.mark_changed();
            if WHEEL_DEBUG {
                eprintln!(
                    "[wheel-debug][slider-route] label={:?} response_hovered={} wheel_points={scroll:.3} value={current:.4}->{next:.4}",
                    text,
                    response.hovered(),
                );
            }
            ui.input_mut(|input| input.smooth_scroll_delta = Vec2::ZERO);
        }
    }
    response
}

fn path_setting_row(ui: &mut egui::Ui, path: &mut Option<PathBuf>, dialog_title: &str) {
    ui.horizontal(|ui| {
        let mut text = path
            .as_ref()
            .map(|path| path.to_string_lossy().into_owned())
            .unwrap_or_default();
        let response = ui.add(
            egui::TextEdit::singleline(&mut text)
                .desired_width((ui.available_width() - 105.0).max(160.0)),
        );
        if response.changed() {
            *path = if text.trim().is_empty() {
                None
            } else {
                Some(PathBuf::from(text.trim()))
            };
        }
        if ui.button("Choose…").clicked() {
            let mut dialog = rfd::FileDialog::new().set_title(dialog_title);
            if let Some(current) = path.as_ref().filter(|path| path.exists()) {
                dialog = dialog.set_directory(current);
            }
            if let Some(selected) = dialog.pick_folder() {
                *path = Some(selected);
            }
        }
    });
}

#[allow(clippy::too_many_arguments)]
fn shortcut_setting_row(
    ui: &mut egui::Ui,
    ctx: &egui::Context,
    label: &str,
    id: &'static str,
    binding: &mut ShortcutBinding,
    default: ShortcutBinding,
    capture: &mut Option<&'static str>,
    captured_binding: Option<ShortcutBinding>,
) {
    if *capture == Some(id) {
        if let Some(captured) = captured_binding {
            *binding = captured;
            *capture = None;
        }
    }

    egui::Grid::new(("shortcut-row", id))
        .num_columns(4)
        .min_col_width(110.0)
        .show(ui, |ui| {
            ui.label(label);
            ui.monospace(binding.display(ctx));
            let changing = *capture == Some(id);
            if ui
                .button(if changing {
                    "Press shortcut…"
                } else {
                    "Change…"
                })
                .clicked()
            {
                *capture = if changing { None } else { Some(id) };
            }
            if ui
                .add_enabled(*binding != default, egui::Button::new("Reset"))
                .clicked()
            {
                *binding = default;
                if *capture == Some(id) {
                    *capture = None;
                }
            }
            ui.end_row();
        });
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

fn zoom_at_pointer(
    zoom: &mut f32,
    pan: &mut Vec2,
    pointer: Pos2,
    canvas_center: Pos2,
    requested_factor: f32,
) {
    let old_zoom = *zoom;
    let new_zoom = (old_zoom * requested_factor).clamp(0.1, 12.0);
    if (new_zoom - old_zoom).abs() <= f32::EPSILON {
        return;
    }
    let actual_factor = new_zoom / old_zoom.max(f32::EPSILON);
    let old_center = canvas_center + *pan;
    *pan += (pointer - old_center) * (1.0 - actual_factor);
    *zoom = new_zoom;
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

#[allow(clippy::too_many_arguments)]
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
            let max = pos2(
                (min.x + cell).min(rect.max.x),
                (min.y + cell).min(rect.max.y),
            );
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

fn screen_to_image_clamped(pos: Pos2, rect: Rect, width: u32, height: u32) -> Option<(f32, f32)> {
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
        pos2(
            rect.min.x + crop.x as f32 * sx,
            rect.min.y + crop.y as f32 * sy,
        ),
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
    hit_crop_handle_with_radius(pointer, rect, 10.0)
}

fn hit_crop_handle_with_radius(pointer: Pos2, rect: Rect, hit_radius: f32) -> Option<CropHandle> {
    crop_handle_positions(rect)
        .into_iter()
        .find(|(_, position)| position.distance(pointer) <= hit_radius)
        .map(|(handle, _)| handle)
}

fn crop_resize_cursor(handle: CropHandle) -> CursorIcon {
    match handle {
        CropHandle::North | CropHandle::South => CursorIcon::ResizeVertical,
        CropHandle::East | CropHandle::West => CursorIcon::ResizeHorizontal,
        CropHandle::NorthWest | CropHandle::SouthEast => CursorIcon::ResizeNwSe,
        CropHandle::NorthEast | CropHandle::SouthWest => CursorIcon::ResizeNeSw,
    }
}

fn radius_resize_cursor(pivot: Pos2, pointer: Pos2) -> CursorIcon {
    let delta = pointer - pivot;
    let abs_x = delta.x.abs();
    let abs_y = delta.y.abs();
    if abs_x > abs_y * 2.0 {
        CursorIcon::ResizeHorizontal
    } else if abs_y > abs_x * 2.0 {
        CursorIcon::ResizeVertical
    } else if delta.x.signum() == delta.y.signum() {
        CursorIcon::ResizeNwSe
    } else {
        CursorIcon::ResizeNeSw
    }
}

fn sprite_crop_preview_rect(image_rect: Rect, allowed_rect: Rect, drag: SpriteCropDrag) -> Rect {
    let delta = drag.current - drag.start;
    let mut left = image_rect.left();
    let mut right = image_rect.right();
    let mut top = image_rect.top();
    let mut bottom = image_rect.bottom();
    let minimum = 2.0;

    if matches!(
        drag.handle,
        CropHandle::NorthWest | CropHandle::West | CropHandle::SouthWest
    ) {
        left = (left + delta.x).clamp(allowed_rect.left(), right - minimum);
    }
    if matches!(
        drag.handle,
        CropHandle::NorthEast | CropHandle::East | CropHandle::SouthEast
    ) {
        right = (right + delta.x).clamp(left + minimum, allowed_rect.right());
    }
    if matches!(
        drag.handle,
        CropHandle::NorthWest | CropHandle::North | CropHandle::NorthEast
    ) {
        top = (top + delta.y).clamp(allowed_rect.top(), bottom - minimum);
    }
    if matches!(
        drag.handle,
        CropHandle::SouthWest | CropHandle::South | CropHandle::SouthEast
    ) {
        bottom = (bottom + delta.y).clamp(top + minimum, allowed_rect.bottom());
    }
    Rect::from_min_max(pos2(left, top), pos2(right, bottom))
}

fn screen_rect_to_uv(rect: Rect, image_rect: Rect) -> Rect {
    let width = image_rect.width().max(f32::EPSILON);
    let height = image_rect.height().max(f32::EPSILON);
    Rect::from_min_max(
        pos2(
            ((rect.left() - image_rect.left()) / width).clamp(0.0, 1.0),
            ((rect.top() - image_rect.top()) / height).clamp(0.0, 1.0),
        ),
        pos2(
            ((rect.right() - image_rect.left()) / width).clamp(0.0, 1.0),
            ((rect.bottom() - image_rect.top()) / height).clamp(0.0, 1.0),
        ),
    )
}

fn paint_sprite_crop_handles(painter: &egui::Painter, rect: Rect) {
    let length = 18.0;
    let half = 10.0;
    let segments = [
        (rect.left_top(), vec2(length, 0.0), vec2(0.0, length)),
        (rect.right_top(), vec2(-length, 0.0), vec2(0.0, length)),
        (rect.right_bottom(), vec2(-length, 0.0), vec2(0.0, -length)),
        (rect.left_bottom(), vec2(length, 0.0), vec2(0.0, -length)),
        (
            pos2(rect.center().x, rect.top()),
            vec2(-half, 0.0),
            vec2(half, 0.0),
        ),
        (
            pos2(rect.center().x, rect.bottom()),
            vec2(-half, 0.0),
            vec2(half, 0.0),
        ),
        (
            pos2(rect.left(), rect.center().y),
            vec2(0.0, -half),
            vec2(0.0, half),
        ),
        (
            pos2(rect.right(), rect.center().y),
            vec2(0.0, -half),
            vec2(0.0, half),
        ),
    ];
    for (origin, first, second) in segments {
        for offset in [vec2(2.0, 2.0), Vec2::ZERO] {
            let color = if offset == Vec2::ZERO {
                Color32::YELLOW
            } else {
                Color32::BLACK
            };
            let stroke = Stroke::new(if offset == Vec2::ZERO { 3.0 } else { 5.0 }, color);
            painter.line_segment([origin + offset, origin + first + offset], stroke);
            painter.line_segment([origin + offset, origin + second + offset], stroke);
        }
    }

    let center_marks = [
        (pos2(rect.center().x, rect.top()), vec2(0.0, length)),
        (pos2(rect.center().x, rect.bottom()), vec2(0.0, -length)),
        (pos2(rect.left(), rect.center().y), vec2(length, 0.0)),
        (pos2(rect.right(), rect.center().y), vec2(-length, 0.0)),
    ];
    for (origin, direction) in center_marks {
        painter.line_segment(
            [origin, origin + direction],
            Stroke::new(3.0, Color32::YELLOW),
        );
    }
}

fn reframe_sprite_from_source(
    source: &RgbaImage,
    old_working: &RgbaImage,
    old_bounds: &CropRegion,
    new_bounds: &CropRegion,
) -> (RgbaImage, RgbaImage) {
    let original = crop_rgba(
        source,
        new_bounds.x,
        new_bounds.y,
        new_bounds.width,
        new_bounds.height,
    );
    let mut working = original.clone();
    let overlap_left = old_bounds.x.max(new_bounds.x);
    let overlap_top = old_bounds.y.max(new_bounds.y);
    let overlap_right = old_bounds
        .x
        .saturating_add(old_working.width())
        .min(new_bounds.x.saturating_add(new_bounds.width));
    let overlap_bottom = old_bounds
        .y
        .saturating_add(old_working.height())
        .min(new_bounds.y.saturating_add(new_bounds.height));

    for source_y in overlap_top..overlap_bottom {
        for source_x in overlap_left..overlap_right {
            let old_x = source_x - old_bounds.x;
            let old_y = source_y - old_bounds.y;
            let new_x = source_x - new_bounds.x;
            let new_y = source_y - new_bounds.y;
            working.put_pixel(new_x, new_y, *old_working.get_pixel(old_x, old_y));
        }
    }
    (original, working)
}

#[allow(clippy::too_many_arguments)]
fn adjust_offsets_after_sprite_reframe(
    offset_x: &mut i32,
    offset_y: &mut i32,
    old_width: u32,
    old_height: u32,
    crop_x: i32,
    crop_y: i32,
    new_width: u32,
    new_height: u32,
) {
    let pivot_x = old_width as f32 / 2.0 + *offset_x as f32;
    let pivot_y = old_height as f32 / 2.0 - *offset_y as f32;
    *offset_x = (pivot_x - crop_x as f32 - new_width as f32 / 2.0).round() as i32;
    *offset_y = (new_height as f32 / 2.0 - (pivot_y - crop_y as f32)).round() as i32;
}

#[cfg(test)]
#[allow(clippy::too_many_arguments)]
fn adjust_offsets_after_sprite_crop(
    offset_x: &mut i32,
    offset_y: &mut i32,
    old_width: u32,
    old_height: u32,
    crop_x: u32,
    crop_y: u32,
    new_width: u32,
    new_height: u32,
) {
    adjust_offsets_after_sprite_reframe(
        offset_x,
        offset_y,
        old_width,
        old_height,
        crop_x as i32,
        crop_y as i32,
        new_width,
        new_height,
    );
}

fn resize_crop_from_drag(
    crop: &mut CropRegion,
    original: &CropRegion,
    handle: CropHandle,
    dx: f32,
    dy: f32,
    image_size: (u32, u32),
    aspect_ratio: Option<f32>,
) {
    let (image_width, image_height) = image_size;
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
        bottom = (original_bottom + dy).clamp(original_top + 1.0, image_height as f32);
    }

    if let Some(aspect) = aspect_ratio.filter(|aspect| aspect.is_finite() && *aspect > 0.0) {
        let proposed_width = (right - left).max(1.0);
        let proposed_height = (bottom - top).max(1.0);
        let horizontal_driver = match handle {
            CropHandle::East | CropHandle::West => true,
            CropHandle::North | CropHandle::South => false,
            _ => {
                ((proposed_width - original.width as f32) / original.width.max(1) as f32).abs()
                    >= ((proposed_height - original.height as f32) / original.height.max(1) as f32)
                        .abs()
            }
        };
        let anchors_right = matches!(
            handle,
            CropHandle::NorthWest | CropHandle::West | CropHandle::SouthWest
        );
        let anchors_bottom = matches!(
            handle,
            CropHandle::NorthWest | CropHandle::North | CropHandle::NorthEast
        );
        let max_width = if anchors_right {
            original_right
        } else {
            image_width as f32 - original_left
        }
        .max(1.0);
        let max_height = if anchors_bottom {
            original_bottom
        } else {
            image_height as f32 - original_top
        }
        .max(1.0);

        let (width, height) = if horizontal_driver {
            let width = proposed_width.min(max_width).min(max_height * aspect);
            (width, width / aspect)
        } else {
            let height = proposed_height.min(max_height).min(max_width / aspect);
            (height * aspect, height)
        };
        left = if anchors_right {
            original_right - width
        } else {
            original_left
        };
        right = left + width;
        top = if anchors_bottom {
            original_bottom - height
        } else {
            original_top
        };
        bottom = top + height;
    }

    let x0 = left
        .round()
        .clamp(0.0, image_width.saturating_sub(1) as f32) as u32;
    let y0 = top
        .round()
        .clamp(0.0, image_height.saturating_sub(1) as f32) as u32;
    let x1 = right.round().clamp((x0 + 1) as f32, image_width as f32) as u32;
    let y1 = bottom.round().clamp((y0 + 1) as f32, image_height as f32) as u32;

    crop.x = x0;
    crop.y = y0;
    crop.width = x1 - x0;
    crop.height = y1 - y0;
}

fn reshape_crop_square(crop: &mut CropRegion, image_width: u32, image_height: u32) {
    set_crop_square_side(crop, crop.width.max(crop.height), image_width, image_height);
}

fn set_crop_square_side(
    crop: &mut CropRegion,
    requested_side: u32,
    image_width: u32,
    image_height: u32,
) {
    let side = requested_side
        .min(image_width.saturating_sub(crop.x).max(1))
        .min(image_height.saturating_sub(crop.y).max(1))
        .max(1);
    crop.width = side;
    crop.height = side;
}

fn set_crop_aspect_from_width(
    crop: &mut CropRegion,
    requested_width: u32,
    aspect: f32,
    image_width: u32,
    image_height: u32,
) {
    let max_width = image_width.saturating_sub(crop.x).max(1);
    let max_height = image_height.saturating_sub(crop.y).max(1);
    let width = (requested_width as f32)
        .min(max_width as f32)
        .min(max_height as f32 * aspect)
        .max(1.0);
    crop.width = width.round().max(1.0) as u32;
    crop.height = (width / aspect).round().max(1.0) as u32;
}

fn set_crop_aspect_from_height(
    crop: &mut CropRegion,
    requested_height: u32,
    aspect: f32,
    image_width: u32,
    image_height: u32,
) {
    let max_width = image_width.saturating_sub(crop.x).max(1);
    let max_height = image_height.saturating_sub(crop.y).max(1);
    let height = (requested_height as f32)
        .min(max_height as f32)
        .min(max_width as f32 / aspect)
        .max(1.0);
    crop.height = height.round().max(1.0) as u32;
    crop.width = (height * aspect).round().max(1.0) as u32;
}

fn constrained_draw_endpoint(
    start: (f32, f32),
    current: (f32, f32),
    image_width: u32,
    image_height: u32,
    aspect: f32,
) -> (f32, f32) {
    let sign_x = if current.0 < start.0 { -1.0 } else { 1.0 };
    let sign_y = if current.1 < start.1 { -1.0 } else { 1.0 };
    let dx = (current.0 - start.0).abs();
    let dy = (current.1 - start.1).abs();
    let max_width = if sign_x < 0.0 {
        start.0
    } else {
        image_width as f32 - start.0
    };
    let max_height = if sign_y < 0.0 {
        start.1
    } else {
        image_height as f32 - start.1
    };
    let width_drives = dx / aspect.max(f32::EPSILON) >= dy;
    let (width, height) = if width_drives {
        let width = dx.min(max_width).min(max_height * aspect);
        (width, width / aspect)
    } else {
        let height = dy.min(max_height).min(max_width / aspect);
        (height * aspect, height)
    };
    (start.0 + sign_x * width, start.1 + sign_y * height)
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
            (100, 100),
            None,
        );
        assert_eq!(
            (changed.x, changed.y, changed.width, changed.height),
            (10, 10, 30, 20)
        );
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
            (100, 100),
            None,
        );
        assert_eq!(
            (changed.x, changed.y, changed.width, changed.height),
            (15, 15, 15, 15)
        );
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
            (100, 100),
            None,
        );
        assert_eq!(
            (changed.x, changed.y, changed.width, changed.height),
            (0, 10, 30, 20)
        );
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

    #[test]
    fn square_resize_keeps_equal_dimensions() {
        let original = crop(10, 10, 20, 10);
        let mut changed = original.clone();
        resize_crop_from_drag(
            &mut changed,
            &original,
            CropHandle::East,
            10.0,
            0.0,
            (100, 100),
            Some(1.0),
        );
        assert_eq!((changed.width, changed.height), (30, 30));
    }

    #[test]
    fn aspect_resize_preserves_original_ratio() {
        let original = crop(10, 10, 20, 10);
        let mut changed = original.clone();
        resize_crop_from_drag(
            &mut changed,
            &original,
            CropHandle::East,
            10.0,
            0.0,
            (100, 100),
            Some(2.0),
        );
        assert_eq!((changed.width, changed.height), (30, 15));
    }

    #[test]
    fn enabling_square_clamps_to_image_bounds() {
        let mut changed = crop(90, 80, 30, 20);
        reshape_crop_square(&mut changed, 100, 100);
        assert_eq!((changed.width, changed.height), (10, 10));
    }

    #[test]
    fn square_draw_endpoint_uses_larger_drag_axis() {
        assert_eq!(
            constrained_draw_endpoint((10.0, 10.0), (40.0, 25.0), 100, 100, 1.0),
            (40.0, 40.0)
        );
    }

    #[test]
    fn pointer_anchored_zoom_keeps_pointed_content_stationary() {
        let canvas_center = pos2(400.0, 300.0);
        let pointer = pos2(525.0, 360.0);
        let mut zoom = 2.0;
        let mut pan = vec2(30.0, -20.0);
        let before = (pointer - (canvas_center + pan)) / zoom;

        zoom_at_pointer(&mut zoom, &mut pan, pointer, canvas_center, 1.5);

        let after = (pointer - (canvas_center + pan)) / zoom;
        assert!((before - after).length() < 0.001);
    }

    #[test]
    fn plain_wheel_is_routed_to_canvas_zoom() {
        let mut input = egui::InputState::default();
        input.events.push(egui::Event::MouseWheel {
            unit: egui::MouseWheelUnit::Line,
            delta: vec2(0.0, 1.0),
            phase: egui::TouchPhase::Move,
            modifiers: egui::Modifiers::NONE,
        });
        let binding = ShortcutSettings::default().sprite_erase_wheel_adjust;

        let (zoom, adjustment) = partition_mouse_wheel_delta(&input, binding, true);

        assert_eq!(zoom, 40.0);
        assert_eq!(adjustment, 0.0);
    }

    #[test]
    fn control_wheel_is_routed_to_active_tool_adjustment() {
        let mut input = egui::InputState::default();
        input.events.push(egui::Event::MouseWheel {
            unit: egui::MouseWheelUnit::Line,
            delta: vec2(0.0, -1.0),
            phase: egui::TouchPhase::Move,
            modifiers: egui::Modifiers::CTRL,
        });
        let binding = ShortcutSettings::default().sprite_erase_wheel_adjust;

        let (zoom, adjustment) = partition_mouse_wheel_delta(&input, binding, true);

        assert_eq!(zoom, 0.0);
        assert_eq!(adjustment, -40.0);
    }

    #[test]
    fn source_and_sprite_drop_areas_are_independent() {
        let mut app = AssetpackBuilderForWonderdraft::from_settings(AppSettings::default());
        app.source_drop_rect = Some(Rect::from_min_max(pos2(0.0, 0.0), pos2(100.0, 100.0)));
        app.sprite_drop_rect = Some(Rect::from_min_max(pos2(0.0, 120.0), pos2(100.0, 220.0)));

        assert_eq!(
            app.drop_target_at(pos2(50.0, 50.0)),
            Some(FileDropTarget::Source)
        );
        assert_eq!(
            app.drop_target_at(pos2(50.0, 150.0)),
            Some(FileDropTarget::Sprite)
        );
        assert_eq!(app.drop_target_at(pos2(150.0, 50.0)), None);
    }

    #[test]
    fn sprite_corner_crop_preview_resizes_both_axes() {
        let rect = Rect::from_min_size(pos2(0.0, 0.0), vec2(100.0, 80.0));
        let preview = sprite_crop_preview_rect(
            rect,
            rect,
            SpriteCropDrag {
                handle: CropHandle::NorthWest,
                start: rect.left_top(),
                current: pos2(20.0, 10.0),
            },
        );
        assert_eq!(
            preview,
            Rect::from_min_max(pos2(20.0, 10.0), pos2(100.0, 80.0))
        );
    }

    #[test]
    fn sprite_crop_preview_can_expand_beyond_current_image() {
        let rect = Rect::from_min_size(pos2(20.0, 10.0), vec2(100.0, 80.0));
        let allowed = Rect::from_min_max(pos2(0.0, 0.0), pos2(150.0, 120.0));
        let preview = sprite_crop_preview_rect(
            rect,
            allowed,
            SpriteCropDrag {
                handle: CropHandle::NorthWest,
                start: rect.left_top(),
                current: pos2(5.0, 2.0),
            },
        );
        assert_eq!(preview.min, pos2(5.0, 2.0));
        assert_eq!(preview.max, rect.max);
    }

    #[test]
    fn expanding_sprite_crop_preserves_edits_and_restores_source_pixels() {
        let mut source = RgbaImage::from_pixel(4, 4, image::Rgba([10, 20, 30, 255]));
        source.put_pixel(0, 0, image::Rgba([1, 2, 3, 255]));
        let old_bounds = crop(1, 1, 2, 2);
        let mut old_working = crop_rgba(&source, 1, 1, 2, 2);
        old_working.put_pixel(0, 0, image::Rgba([99, 88, 77, 0]));
        let new_bounds = crop(0, 0, 4, 4);

        let (original, working) =
            reframe_sprite_from_source(&source, &old_working, &old_bounds, &new_bounds);

        assert_eq!(*original.get_pixel(0, 0), image::Rgba([1, 2, 3, 255]));
        assert_eq!(*working.get_pixel(1, 1), image::Rgba([99, 88, 77, 0]));
        assert_eq!(*working.get_pixel(0, 0), image::Rgba([1, 2, 3, 255]));
    }

    #[test]
    fn sprite_crop_preserves_pivot_location_in_remaining_pixels() {
        let mut offset_x = 10;
        let mut offset_y = -5;
        adjust_offsets_after_sprite_crop(&mut offset_x, &mut offset_y, 100, 80, 10, 5, 70, 60);
        assert_eq!((offset_x, offset_y), (15, -10));
    }
}
