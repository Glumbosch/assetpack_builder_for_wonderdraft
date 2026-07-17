use std::{
    collections::{BTreeMap, HashSet},
    fs::{self, File},
    io::{BufReader, BufWriter},
    path::{Path, PathBuf},
};

use anyhow::{anyhow, Context, Result};
use serde::Serialize;

use crate::image_ops::load_oriented_rgba;
use crate::model::{ProjectFile, SourceImage, SourceRecord, SpriteAsset, SpriteRecord, ThemeDraft};

#[derive(Debug, Default)]
pub struct ExportReport {
    pub png_files: usize,
    pub metadata_files: usize,
    pub theme_files: usize,
    pub warnings: Vec<String>,
}

#[derive(Debug, Serialize)]
struct SymbolMetadata {
    name: String,
    radius: i32,
    offset_x: i32,
    offset_y: i32,
    draw_mode: String,
}

pub fn sanitize_pack_name(value: &str) -> String {
    sanitize_component(value, "MyFantasyPack")
}

pub fn sanitize_file_stem(value: &str) -> String {
    let mut result = String::with_capacity(value.len());
    for ch in value.chars() {
        if ch.is_alphanumeric() || ch == '_' || ch == '-' {
            result.push(ch);
        } else {
            result.push('_');
        }
    }
    let trimmed = result
        .trim_matches(|c| c == '.' || c == ' ' || c == '_')
        .to_owned();
    if trimmed.is_empty() {
        "sprite".to_owned()
    } else {
        trimmed
    }
}

fn sanitize_component(value: &str, fallback: &str) -> String {
    let mut result = String::with_capacity(value.len());
    for ch in value.chars() {
        if ch.is_alphanumeric() || ch == '_' || ch == '-' || ch == ' ' {
            result.push(ch);
        } else {
            result.push('_');
        }
    }
    let trimmed = result.trim_matches(|c| c == '.' || c == ' ').to_owned();
    if trimmed.is_empty() {
        fallback.to_owned()
    } else {
        trimmed
    }
}

fn safe_category_path(category: &str) -> PathBuf {
    let mut result = PathBuf::new();
    for component in category.split(['/', '\\']) {
        let clean = sanitize_component(component, "");
        if !clean.is_empty() {
            result.push(clean);
        }
    }
    if result.as_os_str().is_empty() {
        result.push("Uncategorized");
    }
    result
}

pub fn export_pack(
    root: &Path,
    pack_name: &str,
    sprites: &[SpriteAsset],
    themes: &[ThemeDraft],
) -> Result<ExportReport> {
    let pack_name = sanitize_pack_name(pack_name);
    let mut report = ExportReport::default();
    let mut metadata_by_folder: BTreeMap<PathBuf, BTreeMap<String, SymbolMetadata>> =
        BTreeMap::new();
    let mut destinations = HashSet::new();

    for sprite in sprites {
        let file_stem = sanitize_file_stem(&sprite.file_stem);
        let mut relative_folder = sprite.kind.relative_base(&pack_name);
        relative_folder.push(safe_category_path(&sprite.category));
        let relative_file = relative_folder.join(format!("{file_stem}.png"));

        if !destinations.insert(relative_file.clone()) {
            return Err(anyhow!(
                "Two sprites export to the same file: {}",
                relative_file.display()
            ));
        }

        let destination_folder = root.join(&relative_folder);
        fs::create_dir_all(&destination_folder)
            .with_context(|| format!("Could not create {}", destination_folder.display()))?;
        let destination_file = root.join(&relative_file);
        sprite
            .working
            .save_with_format(&destination_file, image::ImageFormat::Png)
            .with_context(|| format!("Could not save {}", destination_file.display()))?;
        report.png_files += 1;

        if !sprite.working.pixels().any(|pixel| pixel[3] > 0) {
            report.warnings.push(format!(
                "{} contains no visible pixels.",
                destination_file.display()
            ));
        }

        if sprite.kind.is_sprite() {
            metadata_by_folder
                .entry(relative_folder)
                .or_default()
                .insert(
                    file_stem,
                    SymbolMetadata {
                        name: sprite.name.clone(),
                        radius: sprite.radius,
                        offset_x: sprite.offset_x,
                        offset_y: sprite.offset_y,
                        draw_mode: sprite.draw_mode.as_str().to_owned(),
                    },
                );
        }
    }

    for (folder, metadata) in metadata_by_folder {
        let path = root.join(folder).join(".wonderdraft_symbols");
        let file =
            File::create(&path).with_context(|| format!("Could not create {}", path.display()))?;
        serde_json::to_writer_pretty(BufWriter::new(file), &metadata)
            .with_context(|| format!("Could not write {}", path.display()))?;
        report.metadata_files += 1;
    }

    if !themes.is_empty() {
        let theme_folder = root.join("themes");
        fs::create_dir_all(&theme_folder)?;
        let mut used_theme_names = HashSet::new();
        for theme in themes {
            let file_stem = sanitize_file_stem(&theme.file_stem);
            if !used_theme_names.insert(file_stem.clone()) {
                return Err(anyhow!("Duplicate theme filename: {file_stem}"));
            }
            let value: serde_json::Value = serde_json::from_str(&theme.json_text)
                .with_context(|| format!("Theme {file_stem} contains invalid JSON"))?;
            let path = theme_folder.join(format!("{file_stem}.wonderdraft_theme"));
            let file = File::create(&path)?;
            serde_json::to_writer_pretty(BufWriter::new(file), &value)?;
            report.theme_files += 1;
        }
    }

    Ok(report)
}

pub fn save_project(
    project_path: &Path,
    pack_name: &str,
    next_id: u64,
    sources: &[SourceImage],
    sprites: &[SpriteAsset],
    themes: &[ThemeDraft],
) -> Result<()> {
    let project_parent = project_path.parent().unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(project_parent)?;
    let stem = project_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("project");
    let data_folder_name = format!("{stem}.wdassetproj_data");
    let data_folder = project_parent.join(&data_folder_name);
    let sprite_folder = data_folder.join("sprites");
    fs::create_dir_all(&sprite_folder)?;

    let source_records = sources
        .iter()
        .map(|source| SourceRecord {
            id: source.id,
            path: source.path.clone(),
            crops: source.crops.clone(),
            exif_orientation_applied: source.exif_orientation_applied,
            quarter_turns: source.quarter_turns,
        })
        .collect();

    let mut sprite_records = Vec::with_capacity(sprites.len());
    for sprite in sprites {
        let original_rel = PathBuf::from(&data_folder_name)
            .join("sprites")
            .join(format!("{}_original.png", sprite.id));
        let working_rel = PathBuf::from(&data_folder_name)
            .join("sprites")
            .join(format!("{}_working.png", sprite.id));
        sprite
            .original
            .save_with_format(project_parent.join(&original_rel), image::ImageFormat::Png)?;
        sprite
            .working
            .save_with_format(project_parent.join(&working_rel), image::ImageFormat::Png)?;
        sprite_records.push(SpriteRecord {
            id: sprite.id,
            source_id: sprite.source_id,
            crop_id: sprite.crop_id,
            crop_bounds: Some(sprite.crop_bounds.clone()),
            original_file: original_rel,
            working_file: working_rel,
            name: sprite.name.clone(),
            file_stem: sprite.file_stem.clone(),
            kind: sprite.kind,
            category: sprite.category.clone(),
            draw_mode: sprite.draw_mode,
            radius: sprite.radius,
            offset_x: sprite.offset_x,
            offset_y: sprite.offset_y,
        });
    }

    let project = ProjectFile {
        version: 1,
        pack_name: pack_name.to_owned(),
        next_id,
        sources: source_records,
        sprites: sprite_records,
        themes: themes.to_vec(),
    };
    let file = File::create(project_path)?;
    serde_json::to_writer_pretty(BufWriter::new(file), &project)?;
    Ok(())
}

pub struct LoadedProject {
    pub pack_name: String,
    pub next_id: u64,
    pub sources: Vec<SourceImage>,
    pub sprites: Vec<SpriteAsset>,
    pub themes: Vec<ThemeDraft>,
    pub warnings: Vec<String>,
}

pub fn load_project(project_path: &Path) -> Result<LoadedProject> {
    let file = File::open(project_path)?;
    let project: ProjectFile = serde_json::from_reader(BufReader::new(file))?;
    if project.version != 1 {
        return Err(anyhow!(
            "Unsupported project version {} (expected 1)",
            project.version
        ));
    }
    let project_parent = project_path.parent().unwrap_or_else(|| Path::new("."));
    let mut warnings = Vec::new();
    let mut sources = Vec::new();

    for source in project.sources {
        let loaded_image = if source.exif_orientation_applied {
            load_oriented_rgba(&source.path)
        } else {
            image::open(&source.path).map(|image| image.to_rgba8())
        };
        match loaded_image {
            Ok(mut image) => {
                for _ in 0..source.quarter_turns % 4 {
                    image = image::imageops::rotate90(&image);
                }
                sources.push(SourceImage {
                    id: source.id,
                    path: source.path,
                    image,
                    crops: source.crops,
                    exif_orientation_applied: source.exif_orientation_applied,
                    quarter_turns: source.quarter_turns % 4,
                    texture: None,
                })
            }
            Err(error) => warnings.push(format!(
                "Could not reload source image {}: {error}",
                source.path.display()
            )),
        }
    }

    let mut sprites = Vec::new();
    for sprite in project.sprites {
        let original_path = project_parent.join(&sprite.original_file);
        let working_path = project_parent.join(&sprite.working_file);
        let original = image::open(&original_path)
            .with_context(|| format!("Could not open {}", original_path.display()))?
            .to_rgba8();
        let working = image::open(&working_path)
            .with_context(|| format!("Could not open {}", working_path.display()))?
            .to_rgba8();
        let crop_bounds = sprite.crop_bounds.unwrap_or_else(|| {
            sources
                .iter()
                .find(|source| source.id == sprite.source_id)
                .and_then(|source| source.crops.iter().find(|crop| crop.id == sprite.crop_id))
                .cloned()
                .unwrap_or_else(|| {
                    let mut crop = crate::model::CropRegion::new(
                        sprite.crop_id,
                        0,
                        0,
                        original.width(),
                        original.height(),
                    );
                    crop.sprite_id = Some(sprite.id);
                    crop
                })
        });
        sprites.push(SpriteAsset {
            id: sprite.id,
            source_id: sprite.source_id,
            crop_id: sprite.crop_id,
            crop_bounds,
            original,
            working,
            name: sprite.name,
            file_stem: sprite.file_stem,
            kind: sprite.kind,
            category: sprite.category,
            draw_mode: sprite.draw_mode,
            radius: sprite.radius,
            offset_x: sprite.offset_x,
            offset_y: sprite.offset_y,
            texture: None,
            texture_dirty: true,
            undo: Vec::new(),
            redo: Vec::new(),
        });
    }

    Ok(LoadedProject {
        pack_name: project.pack_name,
        next_id: project.next_id,
        sources,
        sprites,
        themes: project.themes,
        warnings,
    })
}
