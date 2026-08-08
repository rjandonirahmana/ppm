//! repository/articles.rs — Artikel halaman depan (migrasi 69). Daftar publik
//! hanya yang `published`; admin melihat draf juga lewat `list_articles_all`.

use anyhow::{Context, Result};
use deadpool_postgres::Pool;
use tokio_postgres::Row;

use crate::models::Article;

const COLS: &str = "id, slug, title, excerpt, body, cover_url, published, created_at";

fn row_to_article(r: Row) -> Article {
    let created: chrono::DateTime<chrono::Utc> = r.get(7);
    Article {
        id: r.get(0),
        slug: r.get(1),
        title: r.get(2),
        excerpt: r.get(3),
        body: r.get(4),
        cover_url: r.get(5),
        published: r.get(6),
        created_at: crate::service::fmt::tanggal_panjang(created),
    }
}

/// Artikel TERBIT, terbaru dulu. `limit` 0 = tanpa batas.
///
/// Halaman depan hanya menampilkan beberapa yang terbaru, sedangkan /artikel
/// menampilkan semuanya — satu fungsi dengan batas, bukan dua query yang harus
/// dijaga tetap mengurutkan dengan cara yang sama.
pub async fn list_articles_published(pool: &Pool, limit: i64) -> Result<Vec<Article>> {
    let c = pool.get().await?;
    let sql = format!(
        "SELECT {COLS} FROM articles WHERE published ORDER BY created_at DESC, id DESC{}",
        if limit > 0 { " LIMIT $1" } else { "" }
    );
    let rows = if limit > 0 {
        c.query(&sql, &[&limit]).await
    } else {
        c.query(&sql, &[]).await
    }
    .context("list_articles_published")?;
    Ok(rows.into_iter().map(row_to_article).collect())
}

/// SEMUA artikel termasuk draf — halaman kelola admin.
pub async fn list_articles_all(pool: &Pool) -> Result<Vec<Article>> {
    let c = pool.get().await?;
    let rows = c
        .query(
            &format!("SELECT {COLS} FROM articles ORDER BY created_at DESC, id DESC"),
            &[],
        )
        .await
        .context("list_articles_all")?;
    Ok(rows.into_iter().map(row_to_article).collect())
}

/// Satu artikel TERBIT berdasarkan slug — halaman publik `/artikel/<slug>`.
/// Draf sengaja tak terjangkau lewat sini: tautannya bisa saja dibagikan
/// sebelum artikelnya siap terbit.
pub async fn get_article_published(pool: &Pool, slug: &str) -> Result<Option<Article>> {
    let c = pool.get().await?;
    let row = c
        .query_opt(
            &format!("SELECT {COLS} FROM articles WHERE slug = $1 AND published"),
            &[&slug],
        )
        .await
        .context("get_article_published")?;
    Ok(row.map(row_to_article))
}

/// Isi artikel yang bisa disunting pengelola. `id` `None` = artikel baru.
pub struct ArticleInput<'a> {
    pub slug: &'a str,
    pub title: &'a str,
    pub excerpt: &'a str,
    pub body: &'a str,
    pub cover_url: Option<&'a str>,
    pub published: bool,
}

/// Berapa akhiran angka yang dicoba sebelum menyerah pada slug yang bentrok.
/// Sepuluh judul yang sama persis sudah jauh melampaui apa pun yang masuk akal
/// untuk satu pondok; batasnya ada supaya loop ini tak mungkin berputar terus.
const MAKS_PERCOBAAN_SLUG: u32 = 10;

/// Simpan artikel baru; balas id-nya.
///
/// Slug bentrok ditangani di sini, bukan dilempar apa adanya: dua kegiatan
/// tahunan dengan judul yang sama itu hal biasa, dan menolak yang kedua dengan
/// "duplicate key" memaksa pengelola mengarang judul lain. Yang kedua jadi
/// `judul-2`, ketiga `judul-3`, dan seterusnya.
///
/// Bentrokan dideteksi dari galat UNIQUE-nya, bukan dari `SELECT` pendahulu:
/// antara memeriksa dan menyisipkan selalu ada celah tempat penyimpanan lain
/// bisa memakai slug yang sama, dan yang menutup celah itu memang indeks unik
/// di tabelnya.
pub async fn insert_article(pool: &Pool, a: &ArticleInput<'_>, created_by: i64) -> Result<i64> {
    let c = pool.get().await?;
    const SQL: &str =
        "INSERT INTO articles (slug, title, excerpt, body, cover_url, published, created_by) \
         VALUES ($1, $2, $3, $4, $5, $6, $7) RETURNING id";

    for n in 1..=MAKS_PERCOBAAN_SLUG {
        // Slug dipotong dulu supaya akhirannya tetap muat di VARCHAR(160).
        // `slugify` hanya menghasilkan ASCII, jadi memotong per-byte aman.
        let slug = if n == 1 {
            a.slug.to_string()
        } else {
            let pangkal = &a.slug[..a.slug.len().min(150)];
            format!("{pangkal}-{n}")
        };
        let hasil = c
            .query_one(
                SQL,
                &[
                    &slug,
                    &a.title,
                    &a.excerpt,
                    &a.body,
                    &a.cover_url,
                    &a.published,
                    &created_by,
                ],
            )
            .await;
        match hasil {
            Ok(row) => return Ok(row.get(0)),
            Err(e) if slug_bentrok(&e) => continue,
            Err(e) => return Err(e).context("insert_article"),
        }
    }
    anyhow::bail!(
        "Sudah ada {MAKS_PERCOBAAN_SLUG} artikel dengan alamat serupa — ubah judulnya."
    )
}

/// Galat ini khusus pelanggaran UNIQUE pada `articles.slug`?
///
/// Kolom uniknya disebut eksplisit: `articles` juga punya primary key, dan
/// menelan SETIAP pelanggaran unik berarti bentrokan id akan tampak seperti
/// judul kembar lalu dicoba lagi sepuluh kali tanpa guna.
fn slug_bentrok(e: &tokio_postgres::Error) -> bool {
    e.code() == Some(&tokio_postgres::error::SqlState::UNIQUE_VIOLATION)
        && e.as_db_error().and_then(|d| d.constraint()).is_some_and(|c| c.contains("slug"))
}

/// Perbarui artikel. Slug TIDAK ikut diubah: alamat yang sudah terbit dan
/// mungkin sudah dibagikan tak boleh berubah hanya karena judulnya dirapikan.
pub async fn update_article(pool: &Pool, id: i64, a: &ArticleInput<'_>) -> Result<bool> {
    let c = pool.get().await?;
    let n = c
        .execute(
            "UPDATE articles SET title = $2, excerpt = $3, body = $4, cover_url = $5, \
                                 published = $6, updated_at = NOW() \
             WHERE id = $1",
            &[&id, &a.title, &a.excerpt, &a.body, &a.cover_url, &a.published],
        )
        .await
        .context("update_article")?;
    Ok(n > 0)
}

pub async fn delete_article(pool: &Pool, id: i64) -> Result<bool> {
    let c = pool.get().await?;
    let n = c
        .execute("DELETE FROM articles WHERE id = $1", &[&id])
        .await
        .context("delete_article")?;
    Ok(n > 0)
}
