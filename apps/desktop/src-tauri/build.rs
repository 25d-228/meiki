fn main() {
    let icon_directory = std::path::Path::new("icons");
    let icon_path = icon_directory.join("icon.png");
    std::fs::create_dir_all(icon_directory)
        .expect("failed to create the development icon directory");
    std::fs::write(
        &icon_path,
        [
            0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x48,
            0x44, 0x52, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00,
            0x00, 0x1f, 0x15, 0xc4, 0x89, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x44, 0x41, 0x54, 0x08,
            0x1d, 0x63, 0xf8, 0xcf, 0xc0, 0xf0, 0x1f, 0x00, 0x05, 0x80, 0x02, 0x3f, 0x49, 0xc2,
            0xc9, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4e, 0x44, 0xae, 0x42, 0x60, 0x82,
        ],
    )
    .expect("failed to write the development icon");

    let mut windows_icon = ico::IconDir::new(ico::ResourceType::Icon);
    let image = ico::IconImage::from_rgba_data(1, 1, vec![45, 102, 77, 255]);
    windows_icon.add_entry(
        ico::IconDirEntry::encode(&image).expect("failed to encode the development Windows icon"),
    );
    let icon_file = std::fs::File::create(icon_directory.join("icon.ico"))
        .expect("failed to create the development Windows icon");
    windows_icon
        .write(icon_file)
        .expect("failed to write the development Windows icon");

    tauri_build::build();
}
