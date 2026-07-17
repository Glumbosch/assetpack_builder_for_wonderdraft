use std::path::PathBuf;

use egui::TextureHandle;
use image::RgbaImage;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DrawMode {
    Normal,
    SampleColor,
    CustomColors,
}

impl DrawMode {
    pub const ALL: [DrawMode; 3] = [
        DrawMode::Normal,
        DrawMode::SampleColor,
        DrawMode::CustomColors,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            DrawMode::Normal => "normal",
            DrawMode::SampleColor => "sample_color",
            DrawMode::CustomColors => "custom_colors",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            DrawMode::Normal => "Normal",
            DrawMode::SampleColor => "Sample ground color",
            DrawMode::CustomColors => "Custom colors (RGB channels)",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AssetKind {
    Symbols,
    Mountains,
    Trees,
    GroundTexture,
    WaterTexture,
    Brush,
}

impl AssetKind {
    pub const ALL: [AssetKind; 6] = [
        AssetKind::Symbols,
        AssetKind::Mountains,
        AssetKind::Trees,
        AssetKind::GroundTexture,
        AssetKind::WaterTexture,
        AssetKind::Brush,
    ];

    pub fn label(self) -> &'static str {
        match self {
            AssetKind::Symbols => "Symbols",
            AssetKind::Mountains => "Mountains",
            AssetKind::Trees => "Trees",
            AssetKind::GroundTexture => "Ground textures",
            AssetKind::WaterTexture => "Water textures",
            AssetKind::Brush => "Brushes",
        }
    }

    pub fn is_sprite(self) -> bool {
        matches!(
            self,
            AssetKind::Symbols | AssetKind::Mountains | AssetKind::Trees
        )
    }

    pub fn relative_base(self, pack_name: &str) -> PathBuf {
        match self {
            AssetKind::Symbols => PathBuf::from("assets")
                .join(pack_name)
                .join("sprites")
                .join("symbols"),
            AssetKind::Mountains => PathBuf::from("assets")
                .join(pack_name)
                .join("sprites")
                .join("mountains"),
            AssetKind::Trees => PathBuf::from("assets")
                .join(pack_name)
                .join("sprites")
                .join("trees"),
            AssetKind::GroundTexture => PathBuf::from("assets")
                .join(pack_name)
                .join("textures")
                .join("ground"),
            AssetKind::WaterTexture => PathBuf::from("assets")
                .join(pack_name)
                .join("textures")
                .join("water"),
            AssetKind::Brush => PathBuf::from("brushes"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CropRegion {
    pub id: u64,
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
    pub sprite_id: Option<u64>,
}

impl CropRegion {
    pub fn new(id: u64, x: u32, y: u32, width: u32, height: u32) -> Self {
        Self {
            id,
            x,
            y,
            width: width.max(1),
            height: height.max(1),
            sprite_id: None,
        }
    }
}

pub struct SourceImage {
    pub id: u64,
    pub path: PathBuf,
    pub image: RgbaImage,
    pub crops: Vec<CropRegion>,
    pub exif_orientation_applied: bool,
    pub quarter_turns: u8,
    pub texture: Option<TextureHandle>,
}

impl SourceImage {
    pub fn display_name(&self) -> String {
        self.path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("Image")
            .to_owned()
    }
}

pub struct SpriteAsset {
    pub id: u64,
    pub source_id: u64,
    pub crop_id: u64,
    pub crop_bounds: CropRegion,
    pub original: RgbaImage,
    pub working: RgbaImage,
    pub name: String,
    pub file_stem: String,
    pub kind: AssetKind,
    pub category: String,
    pub draw_mode: DrawMode,
    pub radius: i32,
    pub offset_x: i32,
    pub offset_y: i32,
    pub texture: Option<TextureHandle>,
    pub texture_dirty: bool,
    pub undo: Vec<SpriteEditSnapshot>,
    pub redo: Vec<SpriteEditSnapshot>,
}

pub struct SpriteEditSnapshot {
    crop_bounds: CropRegion,
    original: RgbaImage,
    working: RgbaImage,
    offset_x: i32,
    offset_y: i32,
}

impl SpriteAsset {
    pub fn push_undo(&mut self) {
        const MAX_HISTORY: usize = 30;
        self.undo.push(self.snapshot());
        if self.undo.len() > MAX_HISTORY {
            self.undo.remove(0);
        }
        self.redo.clear();
    }

    pub fn undo(&mut self) -> bool {
        if let Some(previous) = self.undo.pop() {
            self.redo.push(self.snapshot());
            self.restore_snapshot(previous);
            self.texture_dirty = true;
            true
        } else {
            false
        }
    }

    pub fn redo(&mut self) -> bool {
        if let Some(next) = self.redo.pop() {
            self.undo.push(self.snapshot());
            self.restore_snapshot(next);
            self.texture_dirty = true;
            true
        } else {
            false
        }
    }

    fn snapshot(&self) -> SpriteEditSnapshot {
        SpriteEditSnapshot {
            crop_bounds: self.crop_bounds.clone(),
            original: self.original.clone(),
            working: self.working.clone(),
            offset_x: self.offset_x,
            offset_y: self.offset_y,
        }
    }

    fn restore_snapshot(&mut self, snapshot: SpriteEditSnapshot) {
        self.crop_bounds = snapshot.crop_bounds;
        self.original = snapshot.original;
        self.working = snapshot.working;
        self.offset_x = snapshot.offset_x;
        self.offset_y = snapshot.offset_y;
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThemeDraft {
    pub file_stem: String,
    pub json_text: String,
}

impl Default for ThemeDraft {
    fn default() -> Self {
        Self {
            file_stem: "Adventure".to_owned(),
            json_text: default_theme_json(),
        }
    }
}

pub fn default_theme_json() -> String {
    r#"{
  "water_texture": "Paper",
  "ground_texture": "Feudal",
  "water_hue": 0.02,
  "water_saturation": -0.08,
  "water_value": 0.01,
  "vignette_strength": 0.54,
  "coastal_fx_distance": 22.96,
  "coastline_style": 5,
  "coastline_color": "0.613098,0.765625,0.719152,0.733686",
  "landmass_outline_color": "0.242188,0.217191,0.15799,1",
  "landmass_outline_blend": 1.3,
  "freshwater_color": "0.552887,0.752761,0.773438,0.396706",
  "freshwater_outline_color": "0.039063,0.027906,0.004578,1",
  "ground_colors": [
    "0.902344,0.818217,0.673233,1",
    "0.847656,0.709726,0.562897,1",
    "0.746094,0.734436,0.55957,1",
    "0.84375,0.764494,0.576782,1",
    "0.679688,0.61242,0.49649,1"
  ],
  "water_colors": [
    "0.569641,0.796875,0.722314,1",
    "0.533569,0.710843,0.734375,1",
    "0.422882,0.64827,0.726563,1",
    "0.262177,0.444996,0.554688,1"
  ],
  "water_color_names": ["", "", "", ""],
  "path_color": "0.296875,0.193991,0.10321,1",
  "symbol_custom_colors": [
    "0.335938,0.173587,0.141724,1",
    "0.210938,0.178751,0.12854,1",
    "0.839844,0.794094,0.672531,0.63498"
  ],
  "windrose_color": "0.242188,0.217191,0.15799,0.5",
  "label_presets": {
    "Town": {
      "font_name": "IM FELL DW Pica",
      "font_size": 28,
      "font_color": "0.242188,0.208817,0.120148,1",
      "font_outline_width": 3,
      "font_outline_color": "0.839844,0.794094,0.672531,0.63498"
    },
    "City": {
      "font_name": "IM FELL DW Pica",
      "font_size": 36,
      "font_color": "0.335938,0.173587,0.141724,1",
      "font_outline_width": 3,
      "font_outline_color": "0.839844,0.794094,0.672531,0.63498"
    },
    "Water": {
      "font_name": "IM FELL DW Pica",
      "font_size": 56,
      "font_color": "0.242188,0.208817,0.120148,1",
      "font_outline_width": 4,
      "font_outline_color": "0.687653,0.867188,0.812485,0.505451"
    },
    "Region": {
      "font_name": "IM FELL DW Pica",
      "font_size": 36,
      "font_color": "0.945313,0.901895,0.78653,1",
      "font_outline_width": 2,
      "font_outline_color": "0.210938,0.178751,0.12854,1"
    }
  }
}"#
    .to_owned()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectFile {
    pub version: u32,
    pub pack_name: String,
    pub next_id: u64,
    pub sources: Vec<SourceRecord>,
    pub sprites: Vec<SpriteRecord>,
    pub themes: Vec<ThemeDraft>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceRecord {
    pub id: u64,
    pub path: PathBuf,
    pub crops: Vec<CropRegion>,
    #[serde(default)]
    pub exif_orientation_applied: bool,
    #[serde(default)]
    pub quarter_turns: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpriteRecord {
    pub id: u64,
    pub source_id: u64,
    pub crop_id: u64,
    #[serde(default)]
    pub crop_bounds: Option<CropRegion>,
    pub original_file: PathBuf,
    pub working_file: PathBuf,
    pub name: String,
    pub file_stem: String,
    pub kind: AssetKind,
    pub category: String,
    pub draw_mode: DrawMode,
    pub radius: i32,
    pub offset_x: i32,
    pub offset_y: i32,
}
