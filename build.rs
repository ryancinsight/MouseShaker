/// Generates a multi-size Windows ICO binary from an RGBA8 source image.
///
/// ICO format specification: https://docs.microsoft.com/en-us/previous-versions/ms997538(v=msdn.10)
/// Sizes included: 16, 32, 48, 256 pixels (the standard set Windows Explorer and the shell use).
/// Each frame is stored as a PNG-compressed RGBA image inside the ICO container.
/// Images wider or taller than each target size are downsampled with Lanczos3 for quality.
#[cfg(target_os = "windows")]
fn generate_multi_size_ico(source: &image::RgbaImage) -> Vec<u8> {
    use image::ImageEncoder;
    use image::codecs::png::PngEncoder;
    use image::imageops::FilterType;

    // ICO sizes recommended by Windows shell documentation.
    const SIZES: &[u32] = &[16, 32, 48, 256];

    let mut png_frames: Vec<Vec<u8>> = Vec::with_capacity(SIZES.len());
    for &size in SIZES {
        let frame = if source.width() == size && source.height() == size {
            source.clone()
        } else {
            image::imageops::resize(source, size, size, FilterType::Lanczos3)
        };
        let mut png_bytes: Vec<u8> = Vec::new();
        PngEncoder::new(&mut png_bytes)
            .write_image(frame.as_raw(), size, size, image::ColorType::Rgba8.into())
            .expect("failed to encode ICO PNG frame");
        png_frames.push(png_bytes);
    }

    let count = SIZES.len() as u16;
    // ICO binary layout: 6-byte ICONDIR + N × 16-byte ICONDIRENTRY + image data blobs.
    let data_offset_base: u32 = 6 + 16 * count as u32;
    let mut ico: Vec<u8> = Vec::new();

    // ICONDIR header
    ico.extend_from_slice(&0u16.to_le_bytes()); // reserved
    ico.extend_from_slice(&1u16.to_le_bytes()); // image type = icon
    ico.extend_from_slice(&count.to_le_bytes()); // number of images

    // ICONDIRENTRY table — compute cumulative data offsets
    let mut blob_offset = data_offset_base;
    for (i, &size) in SIZES.iter().enumerate() {
        // Width/height: 0 encodes 256 per ICO spec.
        let dim = if size >= 256 { 0u8 } else { size as u8 };
        ico.push(dim);  // width
        ico.push(dim);  // height
        ico.push(0u8);  // color count (0 = >256 colors)
        ico.push(0u8);  // reserved
        ico.extend_from_slice(&1u16.to_le_bytes());  // color planes
        ico.extend_from_slice(&32u16.to_le_bytes()); // bits per pixel
        ico.extend_from_slice(&(png_frames[i].len() as u32).to_le_bytes()); // data size
        ico.extend_from_slice(&blob_offset.to_le_bytes()); // offset to data
        blob_offset += png_frames[i].len() as u32;
    }

    // Image data blobs (PNG-compressed)
    for frame in &png_frames {
        ico.extend_from_slice(frame);
    }

    ico
}

fn main() {
    #[cfg(target_os = "windows")]
    {
        use image::ImageReader;
        use std::fs;
        use std::path::PathBuf;

        let manifest_dir = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR missing"));
        let source_icon_path = manifest_dir.join("src").join("assets").join("icon.png");
        let out_dir = PathBuf::from(std::env::var("OUT_DIR").expect("OUT_DIR missing"));
        let ico_path = out_dir.join("mouse_shaker.ico");

        println!("cargo:rerun-if-changed={}", source_icon_path.display());

        let source = ImageReader::open(&source_icon_path)
            .expect("failed to open src/assets/icon.png for Windows resource generation")
            .decode()
            .expect("failed to decode src/assets/icon.png for Windows resource generation")
            .into_rgba8();

        let ico_bytes = generate_multi_size_ico(&source);
        fs::write(&ico_path, ico_bytes).expect("failed to write generated Windows .ico resource");

        let resource_icon_path = ico_path.to_string_lossy().replace('\\', "/");

        let mut resource = winresource::WindowsResource::new();
        if std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("gnu")
            && std::process::Command::new("llvm-windres")
                .arg("--version")
                .output()
                .is_ok()
        {
            resource.set_windres_path("llvm-windres");
        }
        // Set human-readable identity strings in the Windows version resource.
        resource.set("ProductName", "Mouse Shaker");
        resource.set("FileDescription", "Mouse Shaker");
        resource.set_icon(&resource_icon_path);
        resource.compile().expect("failed to compile Windows resource icon");
    }
}