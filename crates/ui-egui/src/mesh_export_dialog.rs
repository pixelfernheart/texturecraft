use std::path::{Path, PathBuf};

use egui_phosphor::regular as ic;
use serde_json::json;

use crate::TextureCraftApp;
use crate::i18n::{tr, trf};
use crate::state::{MeshExportDialog, MeshExportEntry, MeshExportPlace};
use crate::theme::Tokens;

const FORMATS: [(&str, &str); 6] = [
    ("obj", "Wavefront OBJ (.obj)"),
    ("gltf", "glTF JSON (.gltf)"),
    ("glb", "glTF binary (.glb)"),
    ("usdz", "USDZ (.usdz)"),
    ("usda", "USD text (.usda)"),
    ("usdc", "USD binary (.usdc)"),
];

pub(crate) fn open(app: &mut TextureCraftApp) {
    if app.ui.windows.mesh_export_dialog.is_some() {
        return;
    }
    let project_directory = app.session.path.as_deref().and_then(|path| Path::new(path).parent()).filter(|path| path.is_dir()).map(Path::to_path_buf);
    let directory = match project_directory {
        Some(directory) => directory,
        None => match std::env::current_dir() {
            Ok(directory) => directory,
            Err(error) => {
                app.toast(trf("Couldn't open {name}: {error}", &[("name", &"current folder"), ("error", &error)]), true);
                return;
            }
        },
    };

    let file_name = format!("{}.obj", texturecraft_engine::formats::export::sanitize_file_stem(&app.session.doc.project.name));
    let location_text = directory.display().to_string();
    let mut dialog = MeshExportDialog {
        project_id: app.session.project_id,
        directory,
        location_text,
        file_name,
        format: "obj".into(),
        places: quick_places(),
        volumes: mounted_volumes(),
        ..MeshExportDialog::default()
    };
    refresh_entries(&mut dialog);
    app.ui.windows.mesh_export_dialog = Some(dialog);
}

pub(crate) fn show(app: &mut TextureCraftApp, ctx: &egui::Context) {
    let Some(mut dialog) = app.ui.windows.mesh_export_dialog.take() else { return };
    if dialog.project_id != app.session.project_id {
        app.toast(tr("The project changed while the dialog was open, so nothing was done"), true);
        return;
    }
    let mut open = true;
    let mut navigate = None;
    let mut selected_file = None;
    let mut typed_location = None;
    let mut save = false;
    let mut cancel = false;
    let mut replace_existing = false;

    egui::Window::new(tr("Export mesh"))
        .id(egui::Id::unique("mesh_export_save_dialog"))
        .order(egui::Order::Foreground)
        .open(&mut open)
        .collapsible(false)
        .resizable(true)
        .default_size(egui::vec2(880.0, 590.0))
        .min_size(egui::vec2(700.0, 460.0))
        .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
        .show(ctx, |ui| {
            let tk = Tokens::get(ui.ctx());
            egui::Frame::NONE.fill(tk.row).corner_radius(tk.radius).inner_margin(egui::Margin::symmetric(10, 7)).show(ui, |ui| {
                ui.horizontal(|ui| {
                    if ui.button("‹").on_hover_text(tr("Parent folder")).clicked() {
                        navigate = dialog.directory.parent().map(Path::to_path_buf);
                    }
                    ui.label(tr("Location"));
                    let response = ui.add(egui::TextEdit::singleline(&mut dialog.location_text).desired_width(f32::INFINITY));
                    if response.lost_focus() && ui.input(|input| input.key_pressed(egui::Key::Enter)) {
                        typed_location = Some(PathBuf::from(dialog.location_text.trim()));
                    }
                });
            });
            ui.add_space(8.0);

            ui.horizontal(|ui| {
                egui::Frame::NONE.fill(tk.panel).corner_radius(tk.radius).inner_margin(8.0).show(ui, |ui| {
                    ui.set_min_width(170.0);
                    ui.set_max_width(190.0);
                    ui.set_min_height(330.0);
                    ui.vertical(|ui| {
                        egui::ScrollArea::vertical().id_salt("mesh_export_places").show(ui, |ui| {
                            ui.label(egui::RichText::new(tr("Locations")).strong().color(tk.text_dim));
                            ui.add_space(4.0);
                            for place in &dialog.places {
                                let selected = place.path == dialog.directory;
                                let response = ui.selectable_label(selected, format!("{}  {}", ic::FOLDER_SIMPLE, place.label));
                                if response.clicked() {
                                    navigate = Some(place.path.clone());
                                }
                            }
                            ui.add_space(12.0);
                            ui.label(egui::RichText::new(tr("Drives")).strong().color(tk.text_dim));
                            ui.add_space(4.0);
                            for volume in &dialog.volumes {
                                let selected = volume.path == dialog.directory;
                                let response = ui.selectable_label(selected, format!("{}  {}", ic::HARD_DRIVE, volume.label));
                                if response.clicked() {
                                    navigate = Some(volume.path.clone());
                                }
                                response.on_hover_text(volume.path.display().to_string());
                            }
                        });
                    });
                });

                egui::Frame::NONE.fill(tk.panel).corner_radius(tk.radius).inner_margin(8.0).show(ui, |ui| {
                    ui.set_min_width(450.0);
                    ui.set_min_height(330.0);
                    ui.vertical(|ui| {
                        ui.horizontal_wrapped(|ui| {
                            let path_parts = breadcrumbs(&dialog.directory);
                            let last = path_parts.len().saturating_sub(1);
                            for (index, part) in path_parts.iter().enumerate() {
                                if index > 0 {
                                    ui.label(egui::RichText::new("/").color(tk.text_dim));
                                }
                                let label = if index == last { egui::RichText::new(&part.label).strong() } else { egui::RichText::new(&part.label) };
                                if ui.small_button(label).clicked() {
                                    navigate = Some(part.path.clone());
                                }
                            }
                        });
                        ui.separator();
                        egui::Frame::NONE.fill(tk.row).inner_margin(egui::Margin::symmetric(6, 4)).show(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.label(egui::RichText::new(tr("Name")).strong().color(tk.text_dim));
                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                    ui.label(egui::RichText::new(tr("Type")).strong().color(tk.text_dim));
                                });
                            });
                        });
                        egui::ScrollArea::vertical().id_salt("mesh_export_entries").auto_shrink([false, true]).show(ui, |ui| {
                            for entry in &dialog.entries {
                                let name = entry.path.file_name().map_or_else(|| entry.path.display().to_string(), |name| name.to_string_lossy().into_owned());
                                let selected = dialog.selected_entry.as_ref() == Some(&entry.path);
                                let response = ui
                                    .horizontal(|ui| {
                                        let icon = if entry.is_directory { ic::FOLDER } else { ic::FILE };
                                        ui.label(egui::RichText::new(icon).color(tk.text_dim));
                                        let name_width = (ui.available_width() - 90.0).max(80.0);
                                        let row = ui.add_sized(
                                            egui::vec2(name_width, ui.spacing().interact_size.y),
                                            egui::Button::new(name).selected(selected).frame(false),
                                        );
                                        let kind = if entry.is_directory {
                                            tr("Folder").to_string()
                                        } else {
                                            entry
                                                .path
                                                .extension()
                                                .and_then(|extension| extension.to_str())
                                                .map(|extension| format!(".{}", extension.to_ascii_lowercase()))
                                                .unwrap_or_else(|| tr("File").to_string())
                                        };
                                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                            ui.label(egui::RichText::new(kind).small().color(tk.text_dim));
                                        });
                                        row
                                    })
                                    .inner;
                                if response.clicked() {
                                    dialog.selected_entry = Some(entry.path.clone());
                                    if entry.is_directory {
                                        navigate = Some(entry.path.clone());
                                    } else {
                                        selected_file = Some(entry.path.clone());
                                    }
                                }
                            }
                        });
                    });
                });
            });
            ui.add_space(8.0);

            ui.horizontal(|ui| {
                ui.label(tr("Name"));
                let response = ui.add(egui::TextEdit::singleline(&mut dialog.file_name).desired_width(360.0));
                if response.changed() {
                    dialog.overwrite_confirmation = false;
                    dialog.error = None;
                    dialog.selected_entry = None;
                }
                if response.changed()
                    && let Some(format) = extension_format(&dialog.file_name)
                {
                    set_format(&mut dialog, format);
                }
                ui.label(tr("File type"));
                egui::ComboBox::from_id_salt("mesh_export_file_type").width(230.0).selected_text(format_label(&dialog.format)).show_ui(ui, |ui| {
                    for (format, label) in FORMATS {
                        if ui.selectable_value(&mut dialog.format, format.to_string(), label).changed() {
                            update_file_name_extension(&mut dialog.file_name, format);
                            dialog.overwrite_confirmation = false;
                            dialog.error = None;
                        }
                    }
                });
            });

            if dialog.overwrite_confirmation {
                ui.colored_label(tk.warning, tr("File already exists. Replace it?"));
            }

            if let Some(error) = &dialog.error {
                ui.colored_label(tk.danger, error);
            }

            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if ui.button(tr("Cancel")).clicked() {
                    cancel = true;
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let label = if dialog.overwrite_confirmation { tr("Replace") } else { tr("Save") };
                    if ui.add(crate::widgets::primary_button(ui, label)).clicked() {
                        save = true;
                        replace_existing = dialog.overwrite_confirmation;
                    }
                });
            });
        });

    if cancel {
        open = false;
    }
    if replace_existing {
        dialog.overwrite_confirmation = false;
    }

    if let Some(path) = navigate {
        navigate_directory(&mut dialog, path);
    } else if let Some(path) = typed_location {
        navigate_directory(&mut dialog, path);
    } else if let Some(path) = selected_file
        && let Some(name) = path.file_name()
    {
        dialog.file_name = name.to_string_lossy().into_owned();
        dialog.overwrite_confirmation = false;
        if let Some(format) = extension_format(&dialog.file_name) {
            set_format(&mut dialog, format);
        }
    }

    if save {
        match output_path(&dialog) {
            Ok(path) => match path.try_exists() {
                Ok(true) if !replace_existing => dialog.overwrite_confirmation = true,
                Ok(_) => {
                    if let Some(result) = app.run("file.exportMesh", json!({"path": path.display().to_string(), "format": dialog.format})) {
                        let path = result.get("path").and_then(|path| path.as_str()).unwrap_or_default().to_string();
                        app.toast(trf("Exported mesh to {path}", &[("path", &path)]), false);
                        return;
                    }
                }
                Err(error) => dialog.error = Some(trf("Couldn't open {name}: {error}", &[("name", &path.display()), ("error", &error)])),
            },
            Err(error) => dialog.error = Some(error),
        }
    }

    if open {
        app.ui.windows.mesh_export_dialog = Some(dialog);
    }
}

fn refresh_entries(dialog: &mut MeshExportDialog) {
    let entries = std::fs::read_dir(&dialog.directory).and_then(|entries| {
        entries
            .map(|entry| {
                let entry = entry?;
                let is_directory = entry.file_type()?.is_dir();
                Ok(MeshExportEntry { path: entry.path(), is_directory })
            })
            .collect::<std::io::Result<Vec<_>>>()
    });

    match entries {
        Ok(mut entries) => {
            entries.sort_by(|left, right| right.is_directory.cmp(&left.is_directory).then_with(|| left.path.file_name().cmp(&right.path.file_name())));
            dialog.entries = entries;
            dialog.error = None;
        }
        Err(error) => {
            dialog.entries.clear();
            dialog.error = Some(trf("Couldn't open {name}: {error}", &[("name", &dialog.directory.display()), ("error", &error)]));
        }
    }
}

fn navigate_directory(dialog: &mut MeshExportDialog, path: PathBuf) {
    if path.is_dir() {
        dialog.directory = path;
        dialog.location_text = dialog.directory.display().to_string();
        dialog.selected_entry = None;
        dialog.overwrite_confirmation = false;
        dialog.error = None;
        refresh_entries(dialog);
    } else {
        dialog.error = Some(trf("Couldn't open {name}: {error}", &[("name", &path.display()), ("error", &"not a folder")]));
    }
}

fn quick_places() -> Vec<MeshExportPlace> {
    let home = std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME")).map(PathBuf::from);
    let mut places = Vec::new();
    if let Some(home) = home.filter(|path| path.is_dir()) {
        places.push(MeshExportPlace { label: tr("Home").into(), path: home.clone() });
        for (label, folder) in [("Desktop", "Desktop"), ("Documents", "Documents"), ("Downloads", "Downloads")] {
            let path = home.join(folder);
            if path.is_dir() {
                places.push(MeshExportPlace { label: tr(label).into(), path });
            }
        }
    }

    #[cfg(target_os = "windows")]
    let computer = {
        let mut drive = std::env::var_os("SystemDrive").unwrap_or_else(|| "C:".into());
        drive.push("\\");
        PathBuf::from(drive)
    };
    #[cfg(not(target_os = "windows"))]
    let computer = PathBuf::from("/");
    if computer.is_dir() {
        places.push(MeshExportPlace { label: tr("Computer").into(), path: computer });
    }
    places
}

fn mounted_volumes() -> Vec<MeshExportPlace> {
    let mut paths = std::collections::BTreeSet::new();

    #[cfg(target_os = "windows")]
    for drive in b'A'..=b'Z' {
        let path = PathBuf::from(format!("{}:\\", char::from(drive)));
        if path.is_dir() {
            paths.insert(path);
        }
    }

    #[cfg(target_os = "macos")]
    {
        collect_child_directories(Path::new("/Volumes"), &mut paths);
    }

    #[cfg(target_os = "linux")]
    {
        if let Ok(mounts) = std::fs::read_to_string("/proc/self/mountinfo") {
            for line in mounts.lines() {
                let Some((before, after)) = line.split_once(" - ") else { continue };
                let mut fields = before.split_whitespace();
                let mountpoint = fields.nth(4).map(decode_mount_path);
                let mut details = after.split_whitespace();
                let filesystem = details.next().unwrap_or_default();
                let source = details.next().unwrap_or_default();
                let Some(mountpoint) = mountpoint else { continue };
                let path = PathBuf::from(mountpoint);
                if path != Path::new("/")
                    && !path.starts_with("/snap")
                    && !path.starts_with("/var/lib")
                    && source.starts_with("/dev/")
                    && !source.starts_with("/dev/loop")
                    && !matches!(filesystem, "squashfs" | "ramfs" | "devtmpfs")
                    && path.is_dir()
                {
                    paths.insert(path);
                }
            }
        }
        for root in ["/media", "/mnt", "/run/media"] {
            collect_child_directories(Path::new(root), &mut paths);
        }
    }

    #[cfg(any(target_os = "freebsd", target_os = "openbsd", target_os = "netbsd", target_os = "dragonfly"))]
    for root in ["/media", "/mnt"] {
        collect_child_directories(Path::new(root), &mut paths);
    }

    let mut volumes: Vec<_> = paths
        .into_iter()
        .map(|path| {
            let label =
                path.file_name().map(|name| name.to_string_lossy().into_owned()).filter(|name| !name.is_empty()).unwrap_or_else(|| path.display().to_string());
            MeshExportPlace { label, path }
        })
        .collect();
    volumes.sort_by_key(|volume| volume.label.to_lowercase());
    volumes
}

fn collect_child_directories(root: &Path, paths: &mut std::collections::BTreeSet<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(root) else { return };
    for entry in entries.flatten() {
        if entry.file_type().is_ok_and(|kind| kind.is_dir()) {
            paths.insert(entry.path());
        }
    }
}

#[cfg(target_os = "linux")]
fn decode_mount_path(path: &str) -> String {
    let bytes = path.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'\\' && index + 3 < bytes.len() && bytes[index + 1..index + 4].iter().all(u8::is_ascii_digit) {
            let octal = &bytes[index + 1..index + 4];
            let value = u16::from(octal[0] - b'0') * 64 + u16::from(octal[1] - b'0') * 8 + u16::from(octal[2] - b'0');
            if let Ok(value) = u8::try_from(value) {
                decoded.push(value);
                index += 4;
                continue;
            }
            decoded.push(bytes[index]);
            index += 1;
        } else {
            decoded.push(bytes[index]);
            index += 1;
        }
    }
    String::from_utf8_lossy(&decoded).into_owned()
}

fn breadcrumbs(path: &Path) -> Vec<MeshExportPlace> {
    let mut current = PathBuf::new();
    let mut result = Vec::new();
    for component in path.components() {
        current.push(component.as_os_str());
        let label = match component {
            std::path::Component::RootDir => tr("Computer").to_string(),
            std::path::Component::Prefix(_) | std::path::Component::Normal(_) => component.as_os_str().to_string_lossy().into_owned(),
            std::path::Component::CurDir => continue,
            std::path::Component::ParentDir => tr("Parent folder").to_string(),
        };
        result.push(MeshExportPlace { label, path: current.clone() });
    }
    result
}

fn output_path(dialog: &MeshExportDialog) -> Result<PathBuf, String> {
    let file_name = dialog.file_name.trim();
    let name_path = Path::new(file_name);
    if file_name.is_empty()
        || file_name == "."
        || file_name == ".."
        || file_name.contains('/')
        || file_name.contains('\\')
        || name_path.file_name().and_then(|name| name.to_str()) != Some(file_name)
    {
        return Err(tr("Enter a file name, not a folder path").to_string());
    }
    if !FORMATS.iter().any(|(format, _)| *format == dialog.format) {
        return Err(format!("Unsupported mesh format `{}`", dialog.format));
    }
    if !dialog.directory.is_dir() {
        return Err(trf("Couldn't open {name}: {error}", &[("name", &dialog.directory.display()), ("error", &"folder is no longer available")]));
    }
    let mut file_name = PathBuf::from(file_name);
    file_name.set_extension(&dialog.format);
    Ok(dialog.directory.join(file_name))
}

fn extension_format(file_name: &str) -> Option<&'static str> {
    let extension = Path::new(file_name).extension()?.to_str()?.to_ascii_lowercase();
    FORMATS.iter().find_map(|(format, _)| (*format == extension).then_some(*format))
}

fn format_label(format: &str) -> &'static str {
    FORMATS.iter().find_map(|(candidate, label)| (*candidate == format).then_some(*label)).unwrap_or(FORMATS[0].1)
}

fn set_format(dialog: &mut MeshExportDialog, format: &str) {
    if FORMATS.iter().any(|(candidate, _)| *candidate == format) {
        dialog.format = format.to_string();
        update_file_name_extension(&mut dialog.file_name, format);
    }
}

fn update_file_name_extension(file_name: &mut String, format: &str) {
    let path = Path::new(file_name);
    let stem = path.file_stem().and_then(|stem| stem.to_str()).unwrap_or(file_name);
    *file_name = format!("{stem}.{format}");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn changing_format_updates_the_filename_and_manual_extension_updates_format() {
        let mut dialog = MeshExportDialog { file_name: "checkout.obj".into(), format: "obj".into(), ..MeshExportDialog::default() };
        set_format(&mut dialog, "gltf");
        assert_eq!(dialog.file_name, "checkout.gltf");
        assert_eq!(extension_format(&dialog.file_name), Some("gltf"));

        dialog.file_name = "manually-named.glb".into();
        let format = extension_format(&dialog.file_name).unwrap_or("obj");
        set_format(&mut dialog, format);
        assert_eq!(dialog.format, "glb");
        assert_eq!(dialog.file_name, "manually-named.glb");
    }

    #[test]
    fn output_path_applies_the_selected_format_and_rejects_folder_paths() {
        let mut dialog =
            MeshExportDialog { directory: std::env::temp_dir(), file_name: "checkout.gltf".into(), format: "glb".into(), ..MeshExportDialog::default() };
        assert_eq!(output_path(&dialog).unwrap().extension().and_then(|ext| ext.to_str()), Some("glb"));
        dialog.file_name = "../outside.obj".into();
        assert!(output_path(&dialog).is_err());
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn mount_path_decoder_handles_escapes_without_overflowing() {
        assert_eq!(decode_mount_path("/media/My\\040Drive"), "/media/My Drive");
        assert_eq!(decode_mount_path("/media/odd\\777name"), "/media/odd\\777name");
    }

    #[cfg(unix)]
    #[test]
    fn breadcrumbs_point_to_each_ancestor_directory() {
        let parts = breadcrumbs(Path::new("/home/example/Textures"));
        assert_eq!(parts.last().map(|part| part.path.as_path()), Some(Path::new("/home/example/Textures")));
        assert_eq!(parts.get(1).map(|part| part.path.as_path()), Some(Path::new("/home")));
    }
}
