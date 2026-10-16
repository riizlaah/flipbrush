//! Pipeline ikonografi Flipbrush — DESIGN.md §5.
//!
//! Membaca `assets/icons/*.svg` (outline, viewBox 24×24), mem-parse dengan
//! `usvg`, mem-flatten semua kurva Bézier menjadi polyline (toleransi
//! sub-pixel), lalu menuliskan `OUT_DIR/icons.rs` berisi enum `Icon` dan data
//! titik statis. Runtime (`icon.rs`) merendernya sebagai `epaint::PathShape`
//! — tessellator egui, tanpa rasterisasi bitmap.
//!
//! Koordinat keluaran adalah satuan viewBox (0..24) TANPA stroke: lebar stroke
//! 1,5 px diterapkan runtime agar tetap 1,5 px pada ukuran ikon berapa pun
//! (§5).

use std::{env, fmt::Write as _, fs, path::PathBuf};

use usvg::{
    Node, Options, Tree,
    tiny_skia_path::{PathSegment, Point},
};

/// Toleransi flatten kurva dalam satuan viewBox (24 unit ≈ 20 px tampilan).
const TOLERANCE: f32 = 0.1;
/// Kedalaman subdivide maksimum per kurva (2^8 pembagian).
const MAX_DEPTH: u8 = 8;
/// Batas sanity check keluaran: semua titik harus di sekitar viewBox 0..24.
const BOUNDS_MARGIN: f32 = 1.0;

fn main() {
    let manifest = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"));
    let icons_dir = manifest.join("../assets/icons");
    println!("cargo:rerun-if-changed={}", icons_dir.display());

    let mut files: Vec<PathBuf> = fs::read_dir(&icons_dir)
        .unwrap_or_else(|e| panic!("tidak bisa membaca {}: {e}", icons_dir.display()))
        .map(|entry| entry.expect("read_dir entry").path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "svg"))
        .collect();
    files.sort();
    assert!(
        !files.is_empty(),
        "assets/icons/*.svg kosong — set wajib ada di DESIGN.md §5"
    );

    let options = Options::default();
    let mut icons: Vec<IconFile> = Vec::with_capacity(files.len());
    for file in &files {
        let stem = file
            .file_stem()
            .expect("nama file punya stem")
            .to_string_lossy()
            .into_owned();
        let data =
            fs::read(file).unwrap_or_else(|e| panic!("tidak bisa membaca {}: {e}", file.display()));
        let tree = Tree::from_data(&data, &options)
            .unwrap_or_else(|e| panic!("SVG tidak valid ({}): {e}", file.display()));

        let mut subs = Vec::new();
        collect_paths(tree.root(), &mut subs);
        assert!(
            !subs.is_empty(),
            "{} tidak punya geometri apa pun (§5)",
            file.display()
        );
        for sub in &subs {
            for &point in &sub.points {
                let (x, y) = (point[0], point[1]);
                assert!(
                    (-BOUNDS_MARGIN..=24.0 + BOUNDS_MARGIN).contains(&x)
                        && (-BOUNDS_MARGIN..=24.0 + BOUNDS_MARGIN).contains(&y),
                    "{}: titik ({x}, {y}) di luar viewBox 0..24",
                    file.display()
                );
            }
        }

        icons.push(IconFile {
            stem,
            variant: pascal_case(&file.file_stem().unwrap().to_string_lossy()),
            subs,
        });
    }

    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR"));
    let generated = emit(&icons);
    fs::write(out_dir.join("icons.rs"), generated).expect("gagal menulis icons.rs");
}

/// Satu file SVG: nama kebab-case, varian enum PascalCase, geometri flatten.
struct IconFile {
    stem: String,
    variant: String,
    subs: Vec<SubPath>,
}

/// Satu subpath ter-flatten.
struct SubPath {
    points: Vec<[f32; 2]>,
    closed: bool,
}

fn pascal_case(stem: &str) -> String {
    stem.split('-')
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
                None => String::new(),
            }
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Baca geometri usvg → flatten
// ---------------------------------------------------------------------------

fn collect_paths(group: &usvg::Group, out: &mut Vec<SubPath>) {
    for node in group.children() {
        match node {
            Node::Group(inner) => collect_paths(inner, out),
            Node::Path(path) => collect_path(path, out),
            Node::Image(_) | Node::Text(_) => {
                panic!("aset ikon hanya boleh berisi bentuk vektor (§5)")
            }
        }
    }
}

fn collect_path(path: &usvg::Path, out: &mut Vec<SubPath>) {
    let transform = path.abs_transform();
    let mut current: Option<SubPath> = None;

    for segment in path.data().segments() {
        match segment {
            PathSegment::MoveTo(point) => {
                if let Some(done) = current.take() {
                    out.push(done);
                }
                let point = map(&transform, point);
                current = Some(SubPath {
                    points: vec![point],
                    closed: false,
                });
            }
            PathSegment::LineTo(point) => {
                if let Some(sub) = current.as_mut() {
                    push_point(&mut sub.points, map(&transform, point));
                }
            }
            PathSegment::QuadTo(control, end) => {
                let control = map(&transform, control);
                let end = map(&transform, end);
                if let Some(sub) = current.as_mut() {
                    let from = *sub.points.last().expect("subpath punya titik awal");
                    flatten_quad(from, control, end, &mut sub.points);
                }
            }
            PathSegment::CubicTo(c1, c2, end) => {
                let c1 = map(&transform, c1);
                let c2 = map(&transform, c2);
                let end = map(&transform, end);
                if let Some(sub) = current.as_mut() {
                    let from = *sub.points.last().expect("subpath punya titik awal");
                    flatten_cubic(from, c1, c2, end, &mut sub.points);
                }
            }
            PathSegment::Close => {
                if let Some(mut sub) = current.take() {
                    sub.closed = true;
                    out.push(sub);
                }
            }
        }
    }

    // Subpath terbuka (tanpa perintah Close) — tutup secara manual bila
    // titik akhirnya kembali ke titik awal agar epaint memakai mode closed.
    if let Some(mut sub) = current.take() {
        if let (Some(&first), Some(&last)) = (sub.points.first(), sub.points.last())
            && squared_dist(first, last) < 1e-4
        {
            sub.closed = true;
            sub.points.pop();
        }
        out.push(sub);
    }
}

fn map(transform: &usvg::Transform, point: Point) -> [f32; 2] {
    if transform.is_identity() {
        return [point.x, point.y];
    }
    let mut points = [point];
    transform.map_points(&mut points);
    [points[0].x, points[0].y]
}

fn push_point(points: &mut Vec<[f32; 2]>, point: [f32; 2]) {
    if let Some(&last) = points.last()
        && squared_dist(last, point) < 1e-6
    {
        return; // buang titik kembar (kecil, tapi bersih)
    }
    points.push(point);
}

fn squared_dist(a: [f32; 2], b: [f32; 2]) -> f32 {
    let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
    dx * dx + dy * dy
}

fn mid(a: [f32; 2], b: [f32; 2]) -> [f32; 2] {
    [(a[0] + b[0]) * 0.5, (a[1] + b[1]) * 0.5]
}

fn dist_to_chord(point: [f32; 2], a: [f32; 2], b: [f32; 2]) -> f32 {
    let (abx, aby) = (b[0] - a[0], b[1] - a[1]);
    let (apx, apy) = (point[0] - a[0], point[1] - a[1]);
    let len = (abx * abx + aby * aby).sqrt();
    if len <= f32::EPSILON {
        return (apx * apx + apy * apy).sqrt();
    }
    (abx * apy - aby * apx).abs() / len
}

fn flatten_quad(from: [f32; 2], control: [f32; 2], to: [f32; 2], out: &mut Vec<[f32; 2]>) {
    fn recurse(
        from: [f32; 2],
        control: [f32; 2],
        to: [f32; 2],
        depth: u8,
        out: &mut Vec<[f32; 2]>,
    ) {
        if depth >= MAX_DEPTH || dist_to_chord(control, from, to) <= TOLERANCE {
            push_point(out, to);
            return;
        }
        let a = mid(from, control);
        let b = mid(control, to);
        let m = mid(a, b);
        recurse(from, a, m, depth + 1, out);
        recurse(m, b, to, depth + 1, out);
    }
    recurse(from, control, to, 0, out);
}

fn flatten_cubic(
    from: [f32; 2],
    c1: [f32; 2],
    c2: [f32; 2],
    to: [f32; 2],
    out: &mut Vec<[f32; 2]>,
) {
    fn recurse(
        from: [f32; 2],
        c1: [f32; 2],
        c2: [f32; 2],
        to: [f32; 2],
        depth: u8,
        out: &mut Vec<[f32; 2]>,
    ) {
        let flat =
            dist_to_chord(c1, from, to) <= TOLERANCE && dist_to_chord(c2, from, to) <= TOLERANCE;
        if depth >= MAX_DEPTH || flat {
            push_point(out, to);
            return;
        }
        let a = mid(from, c1);
        let b = mid(c1, c2);
        let c = mid(c2, to);
        let ab = mid(a, b);
        let bc = mid(b, c);
        let m = mid(ab, bc);
        recurse(from, a, ab, m, depth + 1, out);
        recurse(m, bc, c, to, depth + 1, out);
    }
    recurse(from, c1, c2, to, 0, out);
}

// ---------------------------------------------------------------------------
// Emit kode sumber
// ---------------------------------------------------------------------------

/// Format float 1-2 desimal, selalu dengan titik desimal (literal f32 valid).
fn fmt_f(v: f32) -> String {
    let mut s = format!("{v:.2}");
    while s.ends_with('0') {
        s.pop();
    }
    if s.ends_with('.') {
        s.push('0');
    }
    if s.is_empty() || s == "-" {
        s.push_str("0.0");
    }
    s
}

fn emit(icons: &[IconFile]) -> String {
    let mut out = String::new();
    out.push_str("// Generated oleh build.rs — JANGAN diedit manual.\n");
    out.push_str("// Sumber: DESIGN.md §5 — assets/icons/*.svg → usvg → polyline.\n\n");

    out.push_str("/// Semua ikon outline Flipbrush (§5).\n");
    out.push_str("#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]\n");
    out.push_str("pub enum Icon {\n");
    for icon in icons {
        writeln!(out, "    {},", icon.variant).expect("write ke String tidak gagal");
    }
    out.push_str("}\n\n");

    out.push_str("/// Satu subpath outline (titik ter-flatten, tanpa stroke).\n");
    out.push_str("pub struct SubPath {\n");
    out.push_str("    pub points: &'static [(f32, f32)],\n");
    out.push_str("    pub closed: bool,\n");
    out.push_str("}\n\n");

    out.push_str("/// Geometri lengkap satu ikon.\n");
    out.push_str("pub struct IconDef {\n");
    out.push_str("    pub paths: &'static [SubPath],\n");
    out.push_str("}\n\n");

    out.push_str("impl Icon {\n");
    writeln!(
        out,
        "    /// Set wajib §5 — {} ikon, urut nama file.\n    pub const ALL: [Icon; {}] = [",
        icons.len(),
        icons.len()
    )
    .expect("write ke String tidak gagal");
    for icon in icons {
        writeln!(out, "        Icon::{},", icon.variant).expect("write ke String tidak gagal");
    }
    out.push_str("    ];\n\n");

    out.push_str("    /// Nama kebab-case sesuai file (§10) — juga accessible name (§9).\n");
    out.push_str("    pub fn name(self) -> &'static str {\n        match self {\n");
    for icon in icons {
        writeln!(
            out,
            "            Icon::{} => \"{}\",",
            icon.variant, icon.stem
        )
        .expect("write ke String tidak gagal");
    }
    out.push_str("        }\n    }\n\n");

    out.push_str("    /// Geometri hasil build (lihat `icon.rs` untuk rendering §5).\n");
    out.push_str("    pub fn def(self) -> &'static IconDef {\n        match self {\n");
    for icon in icons {
        writeln!(
            out,
            "            Icon::{} => &{}_DEF,",
            icon.variant,
            icon.variant.to_uppercase()
        )
        .expect("write ke String tidak gagal");
    }
    out.push_str("        }\n    }\n");
    out.push_str("}\n\n");

    for icon in icons {
        let upper = icon.variant.to_uppercase();
        writeln!(
            out,
            "static {upper}_GEOM: &[SubPath] = &[\n    // {}",
            icon.stem
        )
        .expect("write ke String tidak gagal");
        for sub in &icon.subs {
            out.push_str("    SubPath {\n        points: &[\n");
            for &point in &sub.points {
                let (x, y) = (point[0], point[1]);
                writeln!(out, "            ({}, {}),", fmt_f(x), fmt_f(y))
                    .expect("write ke String tidak gagal");
            }
            writeln!(out, "        ],\n        closed: {},\n    }},", sub.closed)
                .expect("write ke String tidak gagal");
        }
        out.push_str("];\n");
        writeln!(
            out,
            "static {upper}_DEF: IconDef = IconDef {{ paths: {upper}_GEOM }};\n"
        )
        .expect("write ke String tidak gagal");
    }

    out
}
