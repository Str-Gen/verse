use base64::Engine;
use reader_core::book_store::DiskBook;
use serde_json::json;

#[test]
fn image_window_evicts_distant_resources_and_reloads_on_return()
-> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::temp_dir().join(format!("image-window-{}.json", uuid::Uuid::new_v4()));
    let blocks: Vec<_> = (0..9).map(|i| json!({"id":format!("b{i}"),"kind":"image","resource":format!("image{i}"),"runs":[]})).collect();
    let pieces: Vec<_> = (0..9).map(|i| json!({"id":format!("p{i}"),"block_id":format!("b{i}"),"kind":"image","text":"Image","sources":[]})).collect();
    let mut resources = Vec::new();
    for i in 0..9 {
        let image = image::RgbaImage::new(20 + i * 10, 30 + i * 7);
        let mut data = std::io::Cursor::new(Vec::new());
        image.write_to(&mut data, image::ImageFormat::Png)?;
        resources.push(json!({"id":format!("image{i}"),"media_type":"image/png","data_url":format!("data:image/png;base64,{}",base64::engine::general_purpose::STANDARD.encode(data.into_inner()))}));
    }
    let book = json!({"schema_version":1,"book_id":"nine-image-window","revision":"v1","title":"Images","documents":[{"id":"d","blocks":blocks}],"pieces":pieces,"chapters":[],"notes":[],"resources":resources});
    std::fs::write(&path, serde_json::to_vec(&book)?)?;
    let disk = DiskBook::open(&path);
    std::fs::remove_file(path)?;
    let disk = disk?;
    let chapter = disk.load_chapter(0)?;
    assert!(chapter.book.resources.is_empty());
    assert_eq!(chapter.image_dimensions.len(), 9);
    for i in 0..9 {
        assert_eq!(
            chapter.image_dimensions[&format!("image{i}")],
            [20 + i * 10, 30 + i * 7]
        );
    }
    for (at, expected) in [
        (4, vec![2, 3, 4, 5, 6]),
        (8, vec![6, 7, 8]),
        (0, vec![0, 1, 2]),
        (4, vec![2, 3, 4, 5, 6]),
    ] {
        let actual: Vec<_> = disk
            .image_resources(at)?
            .into_iter()
            .map(|r| r.id)
            .collect();
        assert_eq!(
            actual,
            expected
                .into_iter()
                .map(|i| format!("image{i}"))
                .collect::<Vec<_>>()
        );
        assert!(disk.load_window(at)?.book.resources.is_empty());
    }
    Ok(())
}
