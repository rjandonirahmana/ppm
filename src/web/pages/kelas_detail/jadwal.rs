//! Tab JADWAL — daftar jadwal berulang kelas + penyuntingan per kartu.
//! Formulir buat/sunting ada di [`super::jadwal_form`].

use super::jadwal_form::{BuatJadwalForm, CustomDatePicker};
use super::kurikulum::PanelKekosongan;

use leptos::prelude::*;

use crate::models::{
    KelasDetail, ScheduleItem,
};
use crate::web::api::{
    delete_schedule_action, update_schedule_action,
};
use crate::web::components::{kartu_grid, AdminOnly, EmptyState};

// ── Tab JADWAL ────────────────────────────────────────────────────────────────

#[component]
pub(super) fn JadwalTab(
    class_id: i64,
    d: KelasDetail,
    refetch: impl Fn() + Copy + Send + 'static,
) -> impl IntoView {
    let schedules = d.schedules.clone();
    let weekly = d.weekly_sessions;
    let avg = d.avg_duration_min;
    let room_opts = StoredValue::new(d.room_options.clone());
    // Materi jadwal HANYA boleh dari kurikulum kelas ini — bukan seluruh daftar
    // materi. Jadwal mengajarkan apa yang direncanakan kelasnya; menawarkan
    // kitab di luar itu membuat progres kurikulum tak pernah tersentuh.
    let dalam_kurikulum: std::collections::HashSet<i64> =
        d.curriculum.iter().map(|c| c.book_id).filter(|b| *b > 0).collect();
    // Jadwal & anggota kelas adalah wewenang WALI KELAS kelas ini,
    // bukan admin saja. Menunjuk seseorang jadi wali kelas berarti menyerahkan
    // kelas itu kepadanya; sebelumnya wali kelas justru mendapati kelasnya
    // sendiri terkunci. Flag-nya dihitung server — lihat
    // KelasDetail::can_manage_jadwal.
    let can_manage = d.can_manage_jadwal;
    let book_opts = StoredValue::new(
        d.book_options
            .iter()
            .filter(|b| dalam_kurikulum.contains(&b.id))
            .cloned()
            .collect::<Vec<_>>(),
    );
    let status = if schedules.is_empty() {
        "Belum diatur"
    } else {
        "Terjadwal"
    };
    view! {
        <div class="space-y-3 stagger">
            // DUA KOLOM di desktop: form pembuat jadwal di kiri (lengket saat
            // menggulir), statistik + daftar jadwal di kanan. Sebelumnya form
            // dibatasi `md:max-w-md` dengan sisi kanannya KOSONG SAMA SEKALI —
            // dan daftar jadwal terdorong jauh ke bawah lipatan, padahal
            // keduanya dipakai bergantian: buat jadwal, lalu periksa hasilnya.
            //
            // `items-start` supaya kolom kiri tak ikut meregang setinggi daftar.
            // Hanya BARIS ATAS yang dua kolom: pembuat jadwal di kiri,
            // statistik di kanan. Daftar jadwal di bawahnya memakai lebar
            // PENUH.
            //
            // Versi sebelumnya menaruh daftar di kolom kanan juga — dan karena
            // form pembuat jadwal biasanya masih berupa satu tombol, seluruh
            // sisi kiri di bawah tombol itu menganga kosong sepanjang daftar.
            // Membagi kolom hanya berguna bila KEDUA sisi memang berisi.
            <div class="md:flex md:items-start md:gap-4">
            <div class="md:w-80 md:shrink-0">
                <AdminOnly can_manage=can_manage apa="membuat atau mengubah jadwal kelas" siapa="admin, ketua, atau wali kelas ini">
                    <BuatJadwalForm class_id=class_id room_options=room_opts refetch=refetch />
                </AdminOnly>
            </div>

            <div class="md:flex-1 md:min-w-0 mt-3 md:mt-0">
            // ── Statistik jadwal (mockup Jadwal Kelas) ──────────────────────
            <div class="grid grid-cols-3 gap-2">
                <div class="ppm-card p-3 text-center">
                    <span class="material-symbols-outlined text-primary">"calendar_month"</span>
                    <p class="text-headline-sm font-bold text-on-background" data-count=weekly.to_string()>
                        {weekly}
                    </p>
                    <p class="text-[10px] text-on-surface-variant tracking-wide">"Sesi / Minggu"</p>
                </div>
                <div class="ppm-card p-3 text-center">
                    <span class="material-symbols-outlined text-primary">"schedule"</span>
                    <p class="text-headline-sm font-bold text-on-background" data-count=avg.to_string()>
                        {avg}
                    </p>
                    <p class="text-[10px] text-on-surface-variant tracking-wide">"Menit Rata²"</p>
                </div>
                <div class="ppm-card p-3 text-center">
                    <span class="material-symbols-outlined text-primary">"autorenew"</span>
                    <p class="text-body-md font-bold text-on-background mt-2">{status}</p>
                    <p class="text-[10px] text-on-surface-variant tracking-wide">"Rutinitas"</p>
                </div>
            </div>

            </div>
            </div>

            <h3 class="text-body-lg font-bold text-on-background pt-1">"Jadwal Terdaftar"</h3>

            {if schedules.is_empty() {
                view! {
                    <EmptyState
                        icon="calendar_month"
                        title="Belum ada jadwal"
                        subtitle="Buat jadwal baru lewat form di atas."
                    />
                }
                    .into_any()
            } else {
                kartu_grid(
                        schedules
                            .into_iter()
                            .map(|s| {
                                view! { <JadwalCard class_id=class_id s=s room_options=room_opts book_options=book_opts can_manage=can_manage refetch=refetch /> }
                                    .into_any()
                            })
                            .collect(),
                    )
                    .into_any()
            }}
        </div>
    }
}

#[component]
fn JadwalCard(
    class_id: i64,
    s: ScheduleItem,
    room_options: StoredValue<Vec<crate::models::RoomOption>>,
    book_options: StoredValue<Vec<crate::models::BookItem>>,
    /// Admin/ketua ATAU wali kelas ini: ubah & hapus jadwal. Wali kelas &
    /// dewan guru tetap boleh memperbarui MATERI yang sedang dibahas (panel di
    /// bawah kartu) — itu wewenang terpisah.
    can_manage: bool,
    refetch: impl Fn() + Copy + Send + 'static,
) -> impl IntoView {
    let sid = s.id;
    // Salinan untuk panel posisi — `s` sendiri sebagian di-move ke closure di bawah.
    let s_posisi = s.clone();
    let busy = RwSignal::new(false);
    let msg = RwSignal::new(Option::<(bool, String)>::None);

    // Form EDIT jadwal (pre-fill dari data sekarang).
    let editing = RwSignal::new(false);
    let e_title = RwSignal::new(s.title.clone());
    let e_start = RwSignal::new(s.start_hm.clone());
    let e_end = RwSignal::new(s.end_hm.clone());
    let e_limit = RwSignal::new(s.limit_hm.clone());
    let e_rec = RwSignal::new(s.recurrence.clone());
    let e_sd = RwSignal::new(s.start_date.clone());
    let e_ed = RwSignal::new(s.end_date.clone());
    let e_cat = RwSignal::new(s.category.clone());
    let e_present = RwSignal::new(s.present_points.clone());
    let e_late = RwSignal::new(s.late_points.clone());
    let e_absent = RwSignal::new(s.absent_points.clone());
    let e_activity = RwSignal::new(s.activity_type.clone());
    let e_izin = RwSignal::new(s.izin_points.clone());
    let e_room = RwSignal::new(s.room_id);
    let e_custom = RwSignal::new(
        s.custom_dates
            .split(',')
            .filter(|x| !x.trim().is_empty())
            .map(|x| x.trim().to_string())
            .collect::<Vec<_>>(),
    );

    let save = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        if busy.get_untracked() {
            return;
        }
        busy.set(true);
        msg.set(None);
        let a = (
            e_title.get_untracked(),
            e_start.get_untracked(),
            e_end.get_untracked(),
            e_limit.get_untracked(),
            e_rec.get_untracked(),
            e_sd.get_untracked(),
            e_ed.get_untracked(),
            e_cat.get_untracked(),
            e_present.get_untracked(),
            e_late.get_untracked(),
            e_absent.get_untracked(),
            e_room.get_untracked(),
            e_custom.get_untracked().join(","),
            e_activity.get_untracked(),
            e_izin.get_untracked(),
        );
        leptos::task::spawn_local(async move {
            match update_schedule_action(
                sid, a.0, a.1, a.2, a.3, a.4, a.5, a.6, a.7, a.8, a.9, a.10, a.11, a.12, a.13, a.14,
            )
            .await
            {
                Ok(_) => {
                    editing.set(false);
                    refetch();
                }
                Err(e) => {
                    let s = e.to_string();
                    msg.set(Some((
                        false,
                        crate::web::components::pesan_galat(&s),
                    )));
                }
            }
            busy.set(false);
        });
    };

    let del = move |_| {
        if busy.get_untracked() {
            return;
        }
        busy.set(true);
        leptos::task::spawn_local(async move {
            let _ = delete_schedule_action(sid).await;
            busy.set(false);
            refetch();
        });
    };

    // Ekstrak field tampilan ke lokal (hindari partial-move `s` ke dalam closure).
    let title_ro = s.title.clone();
    let recurrence_label = s.recurrence_label.clone();
    let time_label = s.time_label.clone();
    let date_label = s.date_label.clone();
    let category_ro = s.category.clone();
    let present_ro = s.present_points.clone();
    let late_ro = s.late_points.clone();
    let absent_ro = s.absent_points.clone();
    let room_ro = s.room_label.clone();
    let field =
        "w-full bg-surface-container border-0 rounded-lg px-3 py-2.5 text-body-sm text-on-surface";
    view! {
        <div class="ppm-card p-4 card-hover anim-in ppm-accent">
            // Judul BOLEH MEMBUNGKUS, bukan dipotong. Dengan `truncate`, "KBM
            // Ngaji Subuh Putra" tampil sebagai "KBM …" — chip kategori dan
            // tombol sunting di sebelahnya merebut lebar lebih dulu karena
            // keduanya `shrink-0`, dan yang mengalah selalu judulnya. Nama
            // jadwal adalah satu-satunya cara membedakan kartu di daftar; itu
            // yang paling tak boleh hilang.
            <div class="flex items-start justify-between gap-2">
                <p class="text-body-md font-bold text-on-background min-w-0 break-words">
                    {title_ro}
                </p>
                <div class="flex items-center gap-2 shrink-0">
                    {(!category_ro.is_empty())
                        .then(|| {
                            view! {
                                <span class="ppm-chip bg-primary/10 text-primary">
                                    {category_ro.clone()}
                                </span>
                            }
                        })}
                    <span class="ppm-chip bg-secondary-container text-primary">
                        {recurrence_label}
                    </span>
                    // Ubah jadwal = admin. Pamong/dewan guru tetap bisa
                    // memperbarui materi lewat panel di bawah kartu ini.
                    {can_manage
                        .then(|| {
                            view! {
                                <button
                                    class="w-8 h-8 rounded-lg bg-surface-container text-primary flex items-center justify-center press"
                                    on:click=move |_| editing.update(|e| *e = !*e)
                                    aria-label="Edit jadwal"
                                >
                                    <span class="material-symbols-outlined text-[18px]">"edit"</span>
                                </button>
                            }
                        })}
                </div>
            </div>

            {move || {
                msg.get()
                    .map(|(ok, t)| {
                        let cls = if ok {
                            "mt-2 p-2 bg-secondary-container text-on-secondary-container rounded-lg text-body-sm anim-in"
                        } else {
                            "mt-2 p-2 bg-error-container text-on-error-container rounded-lg text-body-sm anim-in"
                        };
                        view! { <div class=cls>{t}</div> }
                    })
            }}

            {move || {
                if editing.get() {
                    // ── Form EDIT jadwal (tanggal/jam/recurrence) ──────────
                    view! {
                        <form class="mt-3 space-y-2 anim-in" method="post" on:submit=save>
                            <input
                                type="text"
                                class=field
                                placeholder="Nama jadwal"
                                prop:value=move || e_title.get()
                                on:input=move |ev| e_title.set(event_target_value(&ev))
                            />
                            // KATEGORI per-jadwal dihapus — selalu ikut kategori
                            // kelas. Alasan sama dengan form buat jadwal.
                            <label class="space-y-1 block">
                                <span class="text-[11px] text-on-surface-variant">"Ruang (perangkat RFID)"</span>
                                <select
                                    class=field
                                    on:change=move |ev| e_room.set(event_target_value(&ev).parse().unwrap_or(0))
                                >
                                    <option value="0" selected=move || e_room.get() == 0>
                                        "— Tanpa ruang —"
                                    </option>
                                    {room_options
                                        .get_value()
                                        .into_iter()
                                        .map(|r| {
                                            let val = r.id.to_string();
                                            let sel = move || e_room.get() == r.id;
                                            view! {
                                                <option value=val selected=sel>
                                                    {r.name}
                                                </option>
                                            }
                                        })
                                        .collect_view()}
                                </select>
                            </label>
                            // JENIS KEGIATAN dihapus — server menurunkannya dari
                            // kategori kelas, lihat
                            // `service::kelas::jenis_dari_kategori_kelas`.
                            // Poin kehadiran: semua positif (tepat waktu ditambah,
                            // telat/alpa/izin dikurangi). Kosong = preset kegiatan.
                            <label class="space-y-1 block">
                                <span class="text-[11px] text-on-surface-variant">
                                    "Bonus tepat waktu — ditambah (kosong = preset)"
                                </span>
                                <input
                                    type="number"
                                    min="0"
                                    class=field
                                    placeholder="preset"
                                    prop:value=move || e_present.get()
                                    on:input=move |ev| e_present.set(event_target_value(&ev))
                                />
                            </label>
                            // POTONGAN IZIN dihapus — izin tidak memotong poin
                            // sama sekali (DELTA_SQL: permit → 0).
                            <label class="space-y-1 block">
                                <span class="text-[11px] text-on-surface-variant">
                                    "Potongan jika telat — dikurangi (kosong = 0)"
                                </span>
                                <input
                                    type="number"
                                    min="0"
                                    class=field
                                    placeholder="mis. 5"
                                    prop:value=move || e_late.get()
                                    on:input=move |ev| e_late.set(event_target_value(&ev))
                                />
                            </label>
                            <label class="space-y-1 block">
                                <span class="text-[11px] text-on-surface-variant">
                                    "Potongan jika alpa — dikurangi (kosong = 15)"
                                </span>
                                <input
                                    type="number"
                                    min="0"
                                    class=field
                                    placeholder="mis. 20"
                                    prop:value=move || e_absent.get()
                                    on:input=move |ev| e_absent.set(event_target_value(&ev))
                                />
                            </label>
                            <div class="grid grid-cols-2 gap-2">
                                <label class="space-y-1">
                                    <span class="text-[11px] text-on-surface-variant">"Jam mulai"</span>
                                    <input
                                        type="time"
                                        class=field
                                        prop:value=move || e_start.get()
                                        on:input=move |ev| e_start.set(event_target_value(&ev))
                                        required=true
                                    />
                                </label>
                                <label class="space-y-1">
                                    <span class="text-[11px] text-on-surface-variant">"Jam selesai"</span>
                                    <input
                                        type="time"
                                        class=field
                                        prop:value=move || e_end.get()
                                        on:input=move |ev| e_end.set(event_target_value(&ev))
                                        required=true
                                    />
                                </label>
                            </div>
                            <div class="grid grid-cols-2 gap-2">
                                <label class="space-y-1">
                                    <span class="text-[11px] text-on-surface-variant">"Batas terlambat"</span>
                                    <input
                                        type="time"
                                        class=field
                                        prop:value=move || e_limit.get()
                                        on:input=move |ev| e_limit.set(event_target_value(&ev))
                                    />
                                </label>
                                <label class="space-y-1">
                                    <span class="text-[11px] text-on-surface-variant">"Pengulangan"</span>
                                    <select
                                        class=field
                                        on:change=move |ev| e_rec.set(event_target_value(&ev))
                                    >
                                        <option value="daily" selected=move || e_rec.get() == "daily">"Harian"</option>
                                        <option value="weekly" selected=move || e_rec.get() == "weekly">"Mingguan"</option>
                                        <option value="monthly" selected=move || e_rec.get() == "monthly">"Bulanan"</option>
                                        <option value="once" selected=move || e_rec.get() == "once">"Sekali"</option>
                                        <option value="custom" selected=move || e_rec.get() == "custom">"Tanggal tertentu"</option>
                                    </select>
                                </label>
                            </div>
                            {move || {
                                if e_rec.get() == "custom" {
                                    view! { <CustomDatePicker dates=e_custom /> }
                                        .into_any()
                                } else {
                                    view! {
                                        <div class="grid grid-cols-2 gap-2">
                                            <label class="space-y-1">
                                                <span class="text-[11px] text-on-surface-variant">"Mulai tanggal"</span>
                                                <input
                                                    type="date"
                                                    class=field
                                                    prop:value=move || e_sd.get()
                                                    on:input=move |ev| e_sd.set(event_target_value(&ev))
                                                    required=true
                                                />
                                            </label>
                                            <label class="space-y-1">
                                                <span class="text-[11px] text-on-surface-variant">"Selesai (opsional)"</span>
                                                <input
                                                    type="date"
                                                    class=field
                                                    prop:value=move || e_ed.get()
                                                    on:input=move |ev| e_ed.set(event_target_value(&ev))
                                                />
                                            </label>
                                        </div>
                                    }
                                        .into_any()
                                }
                            }}
                            <div class="grid grid-cols-2 gap-2 pt-1">
                                <button
                                    type="button"
                                    class="py-2.5 rounded-lg border border-outline-variant text-on-surface font-semibold text-body-sm"
                                    on:click=move |_| editing.set(false)
                                >
                                    "Batal"
                                </button>
                                <button
                                    type="submit"
                                    class="py-2.5 rounded-lg bg-primary text-on-primary font-semibold text-body-sm disabled:opacity-60"
                                    disabled=move || busy.get()
                                >
                                    {move || if busy.get() { "Menyimpan…" } else { "Simpan" }}
                                </button>
                            </div>
                        </form>
                    }
                        .into_any()
                } else {
                    // ── Ringkas + Generate/Hapus ───────────────────────────
                    view! {
                        <p class="text-body-sm text-on-surface-variant flex items-center gap-1 mt-1.5">
                            <span class="material-symbols-outlined text-[15px]">"schedule"</span>
                            {time_label.clone()}
                        </p>
                        <p class="text-body-sm text-on-surface-variant flex items-center gap-1">
                            <span class="material-symbols-outlined text-[15px]">"calendar_month"</span>
                            {date_label.clone()}
                        </p>
                        {(!room_ro.is_empty())
                            .then(|| {
                                view! {
                                    <p class="text-body-sm text-on-surface-variant flex items-center gap-1">
                                        <span class="material-symbols-outlined text-[15px]">"room"</span>
                                        {room_ro.clone()}
                                    </p>
                                }
                            })}
                        {(!present_ro.is_empty())
                            .then(|| {
                                view! {
                                    <p class="text-body-sm text-success flex items-center gap-1">
                                        <span class="material-symbols-outlined text-[15px]">"check_circle"</span>
                                        "Tepat waktu: +" {present_ro.clone()} " poin"
                                    </p>
                                }
                            })}
                        {(!late_ro.is_empty())
                            .then(|| {
                                view! {
                                    <p class="text-body-sm text-on-surface-variant flex items-center gap-1">
                                        <span class="material-symbols-outlined text-[15px]">"bolt"</span>
                                        "Telat: -" {late_ro.clone()} " poin"
                                    </p>
                                }
                            })}
                        {(!absent_ro.is_empty())
                            .then(|| {
                                view! {
                                    <p class="text-body-sm text-on-surface-variant flex items-center gap-1">
                                        <span class="material-symbols-outlined text-[15px]">"gpp_bad"</span>
                                        "Alpa: -" {absent_ro.clone()} " poin"
                                    </p>
                                }
                            })}
                        <PosisiBerjalan
                            class_id=class_id
                            schedule_id=sid
                            s=s_posisi.clone()
                            books=book_options
                            refetch=refetch
                        />
                        {can_manage
                            .then(|| {
                                view! {
                                    <div class="flex justify-end mt-3 pt-3 border-t border-outline-variant/40">
                                        <button
                                            class="flex items-center gap-1.5 px-3 py-2 rounded-lg bg-error-container/60 text-error text-body-sm font-semibold press disabled:opacity-60"
                                            disabled=move || busy.get()
                                            on:click=del
                                        >
                                            <span class="material-symbols-outlined text-[18px]">"delete"</span>
                                            "Hapus Jadwal"
                                        </button>
                                    </div>
                                }
                            })}
                    }
                        .into_any()
                }
            }}
        </div>
    }
}

// ── Posisi materi yang SEDANG BERJALAN (migrasi 57) ──────────────────────────

/// Panel kecil di kartu jadwal: materi apa yang sedang dibahas dan sampai
/// halaman/ayat berapa.
///
/// Ditaruh di JADWAL, bukan di tiap sesi: jadwal rutin berjalan berminggu-minggu
/// dan yang ingin dilihat pengelola adalah "sekarang sampai mana", satu angka
/// yang MAJU — bukan menelusuri catatan pertemuan satu per satu.
///
/// Bentuk isiannya mengikuti jenis materi (ayat+surat untuk Qur'an, halaman
/// untuk Hadist), memakai komponen yang sama dengan rentang kurikulum supaya
/// keduanya tak bisa berbeda cara membacanya.
#[component]
pub(super) fn PosisiBerjalan(
    class_id: i64,
    schedule_id: i64,
    s: crate::models::ScheduleItem,
    books: StoredValue<Vec<crate::models::BookItem>>,
    refetch: impl Fn() + Copy + Send + 'static,
) -> impl IntoView {
    let buka = RwSignal::new(false);
    let busy = RwSignal::new(false);
    let msg = RwSignal::new(Option::<String>::None);

    let book = RwSignal::new(s.current_book_id);
    let surah = RwSignal::new(s.current_surah);
    let unit = RwSignal::new(s.current_unit);

    let judul_ro = s.current_book_title.clone();
    let posisi_ro = s.current_label.clone();

    let simpan = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        if busy.get_untracked() {
            return;
        }
        busy.set(true);
        msg.set(None);
        let (b, sr, u) = (book.get_untracked(), surah.get_untracked(), unit.get_untracked());
        leptos::task::spawn_local(async move {
            match crate::web::api::set_schedule_current_action(schedule_id, b, sr, u).await {
                Ok(()) => {
                    buka.set(false);
                    refetch();
                }
                Err(e) => {
                                        msg.set(Some(crate::web::components::pesan_galat(e)));
                }
            }
            busy.set(false);
        });
    };

    view! {
        <div class="mt-3 pt-3 border-t border-outline-variant/50">
            <div class="flex items-center justify-between gap-2">
                <div class="min-w-0">
                    <p class="text-[10px] font-bold tracking-wide text-on-surface-variant">
                        "MATERI DIBAHAS"
                    </p>
                    {if judul_ro.is_empty() {
                        view! {
                            <p class="text-body-sm text-on-surface-variant">"Belum ditentukan"</p>
                        }
                            .into_any()
                    } else {
                        // Posisi milik jadwal INI — beda makna dari posisi di
                        // kartu kurikulum, yang mewakili kemajuan kelas secara
                        // keseluruhan atas materi yang sama.
                        let posisi = posisi_ro.clone();
                        view! {
                            <p class="text-body-sm font-semibold text-on-background truncate">
                                {judul_ro.clone()}
                            </p>
                            {if posisi.is_empty() {
                                view! {
                                    <p class="text-[11px] text-on-surface-variant">
                                        "Posisi jadwal ini belum diisi."
                                    </p>
                                }
                                    .into_any()
                            } else {
                                view! {
                                    <p class="text-[11px] text-primary font-semibold flex items-center gap-1">
                                        <span class="material-symbols-outlined text-[14px]">
                                            "trending_flat"
                                        </span>
                                        "Sudah sampai " {posisi}
                                    </p>
                                }
                                    .into_any()
                            }}
                        }
                            .into_any()
                    }}
                </div>
                <button
                    class="w-8 h-8 rounded-lg bg-surface-container text-primary flex items-center justify-center press shrink-0"
                    on:click=move |_| buka.update(|b| *b = !*b)
                    aria-label="Ubah materi yang dibahas"
                >
                    <span class="material-symbols-outlined text-[18px]">"menu_book"</span>
                </button>
            </div>

            {move || {
                buka.get()
                    .then(|| {
                        view! {
                            <form class="mt-2 space-y-2 anim-in" method="post" on:submit=simpan>
                                {move || {
                                    msg.get()
                                        .map(|t| {
                                            view! {
                                                <div class="p-2 bg-error-container text-on-error-container rounded-lg text-body-sm">
                                                    {t}
                                                </div>
                                            }
                                        })
                                }}
                                <super::kurikulum::PilihMateri
                                    books=books
                                    book=book
                                    on_ganti=move || {
                                        surah.set(0);
                                        unit.set(0);
                                    }
                                />
                                <super::kurikulum::TitikMateri
                                    books=books
                                    book=book
                                    surah=surah
                                    unit=unit
                                />
                                <button
                                    type="submit"
                                    class="w-full py-2.5 rounded-lg bg-primary text-on-primary font-semibold text-body-sm disabled:opacity-60"
                                    disabled=move || busy.get()
                                >
                                    {move || if busy.get() { "Menyimpan…" } else { "Simpan Materi" }}
                                </button>
                            </form>
                        }
                    })
            }}

            // Peta kekosongan materi yang SEDANG dibahas — di sinilah guru
            // memilih posisi berikutnya, jadi di sini pula ia perlu tahu bagian
            // mana yang paling banyak kosong. Mengikuti buku yang SEDANG
            // dipilih di atas (reaktif), bukan yang tersimpan: guru yang sedang
            // menimbang ganti kitab langsung melihat peta kitab itu.
            {move || {
                let bid = book.get();
                (bid > 0)
                    .then(|| view! { <PanelKekosongan class_id=class_id book_id=bid /> })
            }}
        </div>
    }
}
