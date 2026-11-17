//! Verify a copied device library without importing, flushing, or changing files.
use plato_core::{library::Library, metadata::SortMethod, settings::LibrarySettings};
fn main() {
    let path = std::env::args_os().nth(1).expect("copied library root");
    let settings = LibrarySettings {
        path: path.into(),
        content_path: Some("Books".into()),
        ..Default::default()
    };
    let mut library = Library::from_settings(&settings).unwrap();
    let expected = library
        .db
        .values()
        .filter(|b| b.file.path.starts_with("Books"))
        .count();
    library.sort(SortMethod::Series, false);
    let (books, _) = library.list(&settings.path, None, false);
    assert_eq!(books.len(), expected);
    assert!(books.iter().all(|b| b.file.path.starts_with("Books")));
    let series: Vec<_> = books
        .iter()
        .filter(|b| b.series == "A Series of Unfortunate Events")
        .collect();
    assert_eq!(
        series
            .iter()
            .map(|b| b.number.parse::<f64>().unwrap())
            .collect::<Vec<_>>(),
        (1..=13).map(f64::from).collect::<Vec<_>>()
    );
    println!(
        "{} book entries; all 13 series books in numeric order:",
        books.len()
    );
    for book in series {
        println!("{} — {}", book.number, book.title);
    }
}
