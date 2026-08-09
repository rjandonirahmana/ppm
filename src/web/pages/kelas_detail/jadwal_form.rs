//! Formulir jadwal: picker tanggal manual (recurrence 'custom') + form buat
//! jadwal. Dipisah dari [`super::jadwal`] karena kalender bulanannya sendiri
//! sudah sepanjang satu tab penuh.

use leptos::prelude::*;

use crate::web::api::create_schedule_action;
use crate::web::components::FlashMsg;

/// Picker tanggal manual (recurrence 'custom') = KALENDER bulanan ala kalender
/// akademik: klik hari untuk pilih/batal (loncat-loncat), navigasi bulan ‹ ›.
/// Grid dihitung di klien (chrono wasmbind). Dipakai form Buat & Edit jadwal.
#[component]
pub(super) fn CustomDatePicker(dates: RwSignal<Vec<String>>) -> impl IntoView {
    use chrono::{Datelike, NaiveDate, Utc};
    const HARI: [&str; 7] = ["Sen", "Sel", "Rab", "Kam", "Jum", "Sab", "Min"];
    const BULAN: [&str; 12] = [
        "Januari", "Februari", "Maret", "April", "Mei", "Juni", "Juli", "Agustus", "September",
        "Oktober", "November", "Desember",
    ];
    let today = Utc::now().date_naive();
    // Awal tampilan = bulan tanggal terpilih paling awal, else bulan berjalan.
    let init = dates
        .get_untracked()
        .first()
        .and_then(|s| NaiveDate::parse_from_str(s, "%Y-%m-%d").ok())
        .map(|d| (d.year(), d.month()))
        .unwrap_or((today.year(), today.month()));
    let ym = RwSignal::new(init);
    let prev = move |_| {
        ym.update(|(y, m)| {
            if *m == 1 {
                *y -= 1;
                *m = 12;
            } else {
                *m -= 1;
            }
        })
    };
    let next = move |_| {
        ym.update(|(y, m)| {
            if *m == 12 {
                *y += 1;
                *m = 1;
            } else {
                *m += 1;
            }
        })
    };
    let btn = "w-8 h-8 rounded-lg bg-surface-container text-on-surface flex items-center justify-center press";

    view! {
        <div class="rounded-xl bg-surface-container/60 p-3 space-y-2">
            <div class="flex items-center justify-between">
                <button type="button" class=btn on:click=prev aria-label="Bulan sebelumnya">
                    <span class="material-symbols-outlined text-[18px]">"chevron_left"</span>
                </button>
                <p class="text-body-sm font-bold text-on-background">
                    {move || {
                        let (y, m) = ym.get();
                        format!("{} {}", BULAN[(m - 1) as usize], y)
                    }}
                </p>
                <button type="button" class=btn on:click=next aria-label="Bulan berikutnya">
                    <span class="material-symbols-outlined text-[18px]">"chevron_right"</span>
                </button>
            </div>
            <div class="grid grid-cols-7 gap-1">
                {HARI
                    .iter()
                    .map(|h| {
                        view! {
                            <div class="text-center text-[10px] font-bold text-on-surface-variant py-0.5">
                                {*h}
                            </div>
                        }
                    })
                    .collect_view()}
            </div>
            {move || {
                let (y, m) = ym.get();
                let first = NaiveDate::from_ymd_opt(y, m, 1).expect("tgl 1 valid");
                let lead = first.weekday().num_days_from_monday();
                let (ny, nm) = if m == 12 { (y + 1, 1) } else { (y, m + 1) };
                let days = NaiveDate::from_ymd_opt(ny, nm, 1)
                    .and_then(|d| d.pred_opt())
                    .map(|d| d.day())
                    .unwrap_or(30);
                view! {
                    <div class="grid grid-cols-7 gap-1">
                        {(0..lead).map(|_| view! { <div></div> }).collect_view()}
                        {(1..=days)
                            .map(|day| {
                                let iso = format!("{y:04}-{m:02}-{day:02}");
                                let is_today = NaiveDate::from_ymd_opt(y, m, day) == Some(today);
                                let iso_cls = iso.clone();
                                let cls = move || {
                                    let sel = dates.with(|v| v.iter().any(|d| d == &iso_cls));
                                    if sel {
                                        "aspect-square rounded-lg text-body-sm bg-primary text-on-primary font-bold press flex items-center justify-center"
                                    } else if is_today {
                                        "aspect-square rounded-lg text-body-sm ring-1 ring-primary text-primary font-bold press flex items-center justify-center"
                                    } else {
                                        "aspect-square rounded-lg text-body-sm text-on-surface hover:bg-surface-container press flex items-center justify-center"
                                    }
                                };
                                let iso_tog = iso.clone();
                                let toggle = move |_| {
                                    dates
                                        .update(|v| match v.iter().position(|x| x == &iso_tog) {
                                            Some(p) => {
                                                v.remove(p);
                                            }
                                            None => {
                                                v.push(iso_tog.clone());
                                                v.sort();
                                            }
                                        })
                                };
                                view! {
                                    <button type="button" class=cls on:click=toggle>
                                        {day}
                                    </button>
                                }
                            })
                            .collect_view()}
                    </div>
                }
            }}
            {move || {
                let n = dates.get().len();
                let t = if n == 0 {
                    "Klik tanggal di kalender untuk memilih (boleh loncat-loncat).".to_string()
                } else {
                    format!("{n} tanggal dipilih")
                };
                view! { <p class="text-[11px] text-on-surface-variant">{t}</p> }
            }}
        </div>
    }
}

#[component]
pub(super) fn BuatJadwalForm(
    class_id: i64,
    room_options: StoredValue<Vec<crate::models::RoomOption>>,
    refetch: impl Fn() + Copy + Send + 'static,
) -> impl IntoView {
    let open = RwSignal::new(false);
    let title = RwSignal::new(String::new());
    let start_t = RwSignal::new(String::new());
    let end_t = RwSignal::new(String::new());
    let limit_t = RwSignal::new(String::new());
    let recurrence = RwSignal::new("daily".to_string());
    let start_d = RwSignal::new(String::new());
    let end_d = RwSignal::new(String::new());
    let category = RwSignal::new(String::new());
    let activity = RwSignal::new(String::new());
    let present_point = RwSignal::new(String::new());
    let point = RwSignal::new(String::new());
    let absent_point = RwSignal::new(String::new());
    let izin_point = RwSignal::new(String::new());
    let room = RwSignal::new(0i64);
    // Recurrence 'custom' = daftar tanggal manual (loncat-loncat).
    let custom_dates = RwSignal::new(Vec::<String>::new());
    let busy = RwSignal::new(false);
    let msg = RwSignal::new(Option::<(bool, String)>::None);

    let submit = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        if busy.get_untracked() {
            return;
        }
        busy.set(true);
        msg.set(None);
        let args = (
            title.get_untracked(),
            start_t.get_untracked(),
            end_t.get_untracked(),
            limit_t.get_untracked(),
            recurrence.get_untracked(),
            start_d.get_untracked(),
            end_d.get_untracked(),
            category.get_untracked(),
            present_point.get_untracked(),
            point.get_untracked(),
            absent_point.get_untracked(),
            room.get_untracked(),
            custom_dates.get_untracked().join(","),
            activity.get_untracked(),
            izin_point.get_untracked(),
        );
        leptos::task::spawn_local(async move {
            match create_schedule_action(
                class_id, args.0, args.1, args.2, args.3, args.4, args.5, args.6, args.7, args.8,
                args.9, args.10, args.11, args.12, args.13, args.14,
            )
            .await
            {
                Ok(_) => {
                    msg.set(Some((true, "Jadwal dibuat.".into())));
                    title.set(String::new());
                    start_t.set(String::new());
                    end_t.set(String::new());
                    limit_t.set(String::new());
                    start_d.set(String::new());
                    end_d.set(String::new());
                    category.set(String::new());
                    activity.set(String::new());
                    present_point.set(String::new());
                    point.set(String::new());
                    absent_point.set(String::new());
                    izin_point.set(String::new());
                    room.set(0);
                    custom_dates.set(Vec::new());
                    refetch();
                }
                Err(e) => {
                    let m = e.to_string();
                    msg.set(Some((
                        false,
                        crate::web::components::pesan_galat(&m),
                    )));
                }
            }
            busy.set(false);
        });
    };

    let field =
        "w-full bg-surface-container border-0 rounded-xl px-3 py-2.5 text-body-sm text-on-surface";
    view! {
        {move || {
            if !open.get() {
                return view! {
                    <button
                        class="w-full py-3.5 rounded-2xl bg-primary text-on-primary font-bold flex items-center justify-center gap-2 press"
                        on:click=move |_| open.set(true)
                    >
                        <span class="material-symbols-outlined">"add"</span>
                        "Buat Jadwal Baru"
                    </button>
                }
                    .into_any();
            }
            let field = field;
            view! {
                <form
                    class="ppm-card p-4 space-y-3 anim-in"
                    method="post"
                    on:submit=submit
                >
                    <h3 class="text-body-md font-bold text-on-background">"Jadwal Baru"</h3>
                    <FlashMsg pesan=msg />
                    <input
                        type="text"
                        class=field
                        placeholder="Nama jadwal (mis. Ngaji Subuh)"
                        prop:value=move || title.get()
                        on:input=move |ev| title.set(event_target_value(&ev))
                    />
                    // KATEGORI per-jadwal DIHAPUS dari form. Kategori efektif
                    // sebuah sesi = COALESCE(jadwal.category, kelas.category), dan
                    // kelasnya sudah menyatakan kategorinya sejak dibuat —
                    // menanyakannya lagi per jadwal cuma mengundang dua jawaban
                    // yang berbeda untuk hal yang sama. Nilainya dikirim kosong,
                    // jadi selalu ikut kelas.
                    <label class="space-y-1 block">
                        <span class="text-label-md text-on-surface-variant">
                            "Ruang = perangkat RFID (opsional — daftarkan dulu di User Control)"
                        </span>
                        <select
                            class=field
                            on:change=move |ev| room.set(event_target_value(&ev).parse().unwrap_or(0))
                        >
                            <option value="0">"— Tanpa ruang —"</option>
                            {room_options
                                .get_value()
                                .into_iter()
                                .map(|r| {
                                    let val = r.id.to_string();
                                    view! { <option value=val>{r.name}</option> }
                                })
                                .collect_view()}
                        </select>
                    </label>
                    // JENIS KEGIATAN DIHAPUS dari form — TAPI nilainya tidak
                    // hilang: server menurunkannya dari KATEGORI KELAS (kbm →
                    // kbm, non_kbm → non_kbm). Kelas sudah menyatakannya, jadi
                    // menanyakan lagi hanya membuka peluang keduanya berbeda —
                    // dan yang menentukan preset poin justru yang di jadwal.
                    // Lihat `service::kelas` (jenis_dari_kategori_kelas).
                    // Poin: SEMUA angka positif. Tepat waktu DITAMBAH; telat/alpa/
                    // izin DIKURANGI. Kosong = preset jenis kegiatan di atas.
                    <div class="rounded-xl bg-surface-container/60 p-3 space-y-2">
                        <p class="text-[11px] font-bold tracking-wider text-on-surface-variant">
                            "POIN KEHADIRAN (kosong = ikut preset kategori kelas)"
                        </p>
                        <label class="space-y-1 block">
                            <span class="text-label-md text-on-surface-variant">
                                "Bonus jika tepat waktu — ditambah"
                            </span>
                            <input
                                type="number"
                                min="0"
                                class=field
                                placeholder="preset"
                                prop:value=move || present_point.get()
                                on:input=move |ev| present_point.set(event_target_value(&ev))
                            />
                        </label>
                        // POTONGAN IZIN DIHAPUS: izin TIDAK memotong poin sama
                        // sekali (lihat DELTA_SQL). Santri yang mengurus izinnya
                        // dengan benar tak dihukum seperti yang menghilang tanpa
                        // kabar — kolomnya pun tak lagi dibaca siapa pun.
                        <label class="space-y-1 block">
                            <span class="text-label-md text-on-surface-variant">
                                "Potongan jika telat — dikurangi (kosong = default 0)"
                            </span>
                            <input
                                type="number"
                                min="0"
                                class=field
                                placeholder="mis. 5"
                                prop:value=move || point.get()
                                on:input=move |ev| point.set(event_target_value(&ev))
                            />
                        </label>
                        <label class="space-y-1 block">
                            <span class="text-label-md text-on-surface-variant">
                                "Potongan jika alpa — dikurangi (kosong = default 15)"
                            </span>
                            <input
                                type="number"
                                min="0"
                                class=field
                                placeholder="mis. 20"
                                prop:value=move || absent_point.get()
                                on:input=move |ev| absent_point.set(event_target_value(&ev))
                            />
                        </label>
                    </div>
                    <div class="grid grid-cols-2 gap-2">
                        <label class="space-y-1">
                            <span class="text-label-md text-on-surface-variant">"Jam mulai"</span>
                            <input
                                type="time"
                                class=field
                                prop:value=move || start_t.get()
                                on:input=move |ev| start_t.set(event_target_value(&ev))
                                required=true
                            />
                        </label>
                        <label class="space-y-1">
                            <span class="text-label-md text-on-surface-variant">"Jam selesai"</span>
                            <input
                                type="time"
                                class=field
                                prop:value=move || end_t.get()
                                on:input=move |ev| end_t.set(event_target_value(&ev))
                                required=true
                            />
                        </label>
                    </div>
                    <div class="grid grid-cols-2 gap-2">
                        <label class="space-y-1">
                            <span class="text-label-md text-on-surface-variant">"Batas terlambat"</span>
                            <input
                                type="time"
                                class=field
                                prop:value=move || limit_t.get()
                                on:input=move |ev| limit_t.set(event_target_value(&ev))
                            />
                        </label>
                        <label class="space-y-1">
                            <span class="text-label-md text-on-surface-variant">"Pengulangan"</span>
                            <select
                                class=field
                                on:change=move |ev| recurrence.set(event_target_value(&ev))
                            >
                                <option value="daily">"Harian"</option>
                                <option value="weekly">"Mingguan"</option>
                                <option value="monthly">"Bulanan"</option>
                                <option value="once">"Sekali"</option>
                                <option value="custom">"Tanggal tertentu"</option>
                            </select>
                        </label>
                    </div>
                    // Pola biasa → rentang mulai/selesai. 'Tanggal tertentu' →
                    // picker tanggal manual (loncat-loncat) via kalender native.
                    {move || {
                        if recurrence.get() == "custom" {
                            view! { <CustomDatePicker dates=custom_dates /> }
                                .into_any()
                        } else {
                            view! {
                                <div class="grid grid-cols-2 gap-2">
                                    <label class="space-y-1">
                                        <span class="text-label-md text-on-surface-variant">"Mulai tanggal"</span>
                                        <input
                                            type="date"
                                            class=field
                                            prop:value=move || start_d.get()
                                            on:input=move |ev| start_d.set(event_target_value(&ev))
                                            required=true
                                        />
                                    </label>
                                    <label class="space-y-1">
                                        <span class="text-label-md text-on-surface-variant">"Selesai (opsional)"</span>
                                        <input
                                            type="date"
                                            class=field
                                            prop:value=move || end_d.get()
                                            on:input=move |ev| end_d.set(event_target_value(&ev))
                                        />
                                    </label>
                                </div>
                            }
                                .into_any()
                        }
                    }}
                    <div class="grid grid-cols-2 gap-3">
                        <button
                            type="button"
                            class="py-3 rounded-xl border border-outline-variant text-on-surface font-semibold text-body-sm"
                            on:click=move |_| open.set(false)
                        >
                            "Batal"
                        </button>
                        <button
                            type="submit"
                            class="py-3 rounded-xl bg-primary text-on-primary font-semibold text-body-sm disabled:opacity-60"
                            disabled=move || busy.get()
                        >
                            {move || if busy.get() { "Menyimpan…" } else { "Simpan Jadwal" }}
                        </button>
                    </div>
                </form>
            }
                .into_any()
        }}
    }
}
