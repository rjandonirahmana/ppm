//! Tab SANTRI — anggota kelas: daftar, keluarkan, dan tambah lewat pencarian.

use leptos::prelude::*;

use crate::models::{
    BookProgressItem, KelasDetail, StudentSearchItem,
};
use crate::web::api::{
    add_members_action, angkatan_tersedia_data, remove_member_action, staff_search_students,
    student_book_progress_for_viewer,
};
use crate::web::components::AdminOnly;
use crate::web::components::{BookProgressDetail, EmptyState, Sheet};

// ── Tab SANTRI ────────────────────────────────────────────────────────────────

#[component]
pub(super) fn SantriTab(
    class_id: i64,
    d: KelasDetail,
    refetch: impl Fn() + Copy + Send + 'static,
) -> impl IntoView {
    // Jadwal & anggota kelas kini juga wewenang PAMONG kelas ini, bukan admin
    // saja (wali kelas tetap tidak). Flag-nya dihitung server — lihat
    // KelasDetail::can_manage_jadwal.
    let can_manage = d.can_manage_jadwal;
    // Kelas KBM punya aturan yang tak berlaku di kelas lain: satu santri hanya
    // boleh satu (trigger migrasi 65). Pemilih santri perlu tahu ini supaya
    // bisa memperingatkan SEBELUM penambahannya ditolak database.
    let kelas_kbm = d.category == "kbm";
    let nama_kelas = d.name.clone();
    let members = StoredValue::new(d.members.clone());
    let total = d.members.len();
    let query = RwSignal::new(String::new());
    let busy = RwSignal::new(Option::<i64>::None);

    // Detail progres materi santri (sheet).
    let detail_student = RwSignal::new(Option::<(i64, String)>::None);
    let detail_data = Resource::new(
        move || detail_student.get(),
        |st| async move {
            if let Some((sid, _)) = st {
                student_book_progress_for_viewer(sid).await.ok()
            } else {
                None
            }
        },
    );

    let remove = move |sid: i64| {
        if busy.get_untracked().is_some() {
            return;
        }
        busy.set(Some(sid));
        leptos::task::spawn_local(async move {
            let _ = remove_member_action(class_id, sid).await;
            busy.set(None);
            refetch();
        });
    };

    view! {
        <div class="space-y-3 stagger">
            <p class="text-body-sm text-on-surface-variant">
                "Total " <b class="text-on-background">{total}</b> " santri dalam kelas ini"
            </p>

            // DUA KOLOM di desktop: form tambah + pencarian di kiri, daftar
            // anggota di kanan. Sebelumnya form dibatasi `md:max-w-md` dan
            // daftar diletakkan DI BAWAHNYA — pada layar lebar itu menyisakan
            // separuh layar kosong di sebelah kanan form, sementara daftar
            // anggota terdorong jauh ke bawah lipatan. Padahal keduanya dipakai
            // bergantian: mencari santri, menambahkannya, lalu memastikan ia
            // muncul di daftar.
            //
            // `items-start` supaya kolom kiri tak ikut meregang setinggi daftar
            // anggota yang bisa jauh lebih panjang.
            <div class="md:grid md:grid-cols-12 md:gap-5 md:items-start space-y-3 md:space-y-0">
            <div class="space-y-3 md:col-span-5 md:sticky md:top-4">
            // Santri masuk KELAS (migrasi 61), bukan jadwal — jadi tak perlu
            // lagi menunggu ada jadwal sebelum anggota bisa ditambahkan.
            <AdminOnly can_manage=can_manage apa="menambah atau mengeluarkan santri dari kelas" siapa="admin, ketua, atau pamong kelas ini">
                <AddMemberForm class_id=class_id kelas_kbm=kelas_kbm nama_kelas=nama_kelas.clone() refetch=refetch />
            </AdminOnly>

            // Cari peserta (filter klien)
            {(total > 0)
                .then(|| {
                    view! {
                        <div class="relative">
                            <span class="material-symbols-outlined absolute left-3 top-1/2 -translate-y-1/2 text-outline text-[20px]">
                                "search"
                            </span>
                            <input
                                type="text"
                                class="w-full pl-10 pr-3 py-2.5 bg-surface-container border-0 rounded-xl text-body-sm text-on-surface"
                                placeholder="Cari nama atau NIS santri…"
                                prop:value=move || query.get()
                                on:input=move |ev| query.set(event_target_value(&ev))
                            />
                        </div>
                    }
                })}
            </div>

            // Daftar anggota — kolom kanan di desktop.
            <div class="md:col-span-7">
            {move || {
                let q = query.get().to_lowercase();
                let list: Vec<_> = members
                    .get_value()
                    .into_iter()
                    .filter(|m| {
                        q.is_empty() || m.name.to_lowercase().contains(&q) || m.nis.contains(&q)
                    })
                    .collect();
                if list.is_empty() {
                    return view! {
                        <EmptyState
                            icon="group"
                            title=if total == 0 {
                                "Belum ada santri di kelas ini."
                            } else {
                                "Tidak ada santri yang cocok."
                            }
                        />
                    }
                        .into_any();
                }
                view! {
                    <div class="ppm-card-grid">
                        {list.into_iter()
                    .map(|m| {
                        let sid = m.id;
                        let name = m.name.clone();
                        let initial = name.chars().next().unwrap_or('S').to_string();
                        // Tiga keterangan yang membedakan santri: nama (di
                        // atas), NIS, dan angkatan. Nama saja tak cukup — pada
                        // daftar induk ada 90 nama yang muncul lebih dari sekali.
                        let meta = format!("NIS: {}", m.nis);
                        let ang = m.angkatan.clone();
                        view! {
                            <div class="ppm-card p-3 flex items-start gap-3 card-hover anim-in ppm-accent">
                                <div class="w-10 h-10 rounded-full bg-secondary-container flex items-center justify-center text-primary font-bold shrink-0">
                                    {initial}
                                </div>
                                <div class="flex-1 min-w-0">
                                    <p class="text-body-md font-semibold text-on-background truncate">
                                        {name.clone()}
                                    </p>
                                    <p class="text-body-sm text-on-surface-variant">{meta}</p>
                                    {(!ang.is_empty())
                                        .then(|| {
                                            view! {
                                                <span class="inline-block mt-1 px-2 py-0.5 rounded-full bg-secondary-container text-primary text-[10px] font-bold">
                                                    "Angkatan " {ang}
                                                </span>
                                            }
                                        })}
                                </div>
                                <div class="flex items-center gap-1.5 shrink-0">
                                    <button
                                        class="w-9 h-9 rounded-lg bg-secondary-container text-primary flex items-center justify-center press"
                                        on:click=move |_| {
                                            detail_student.set(Some((sid, name.clone())));
                                        }
                                        aria-label="Lihat progres materi"
                                    >
                                        <span class="material-symbols-outlined text-[18px]">"auto_stories"</span>
                                    </button>
                                    // Mengeluarkan santri = wewenang admin/ketua
                                    // atau pamong kelas ini; tombolnya tak
                                    // ditampilkan sama sekali untuk yang lain.
                                    {can_manage
                                        .then(|| {
                                            view! {
                                                <button
                                                    class="w-9 h-9 rounded-lg bg-error-container/60 text-error flex items-center justify-center press disabled:opacity-50"
                                                    disabled=move || busy.get() == Some(sid)
                                                    on:click=move |_| remove(sid)
                                                    aria-label="Keluarkan dari kelas"
                                                >
                                                    <span class="material-symbols-outlined text-[20px]">"person_remove"</span>
                                                </button>
                                            }
                                        })}
                                </div>
                            </div>
                        }
                    })
                    .collect_view()}
                    </div>
                }
                    .into_any()
            }}
            </div>
            </div>

            // ── Bottom-sheet detail progres materi ─────────────────────────────
            {move || {
                detail_student
                    .get()
                    .map(|(_sid, name)| {
                        view! {
                            <Sheet
                                title="Detail Progres Materi"
                                on_close=move || detail_student.set(None)
                            >
                                <Suspense fallback=|| {
                                    view! {
                                        <div class="space-y-3 animate-pulse">
                                            <div class="h-40 bg-surface-container rounded-2xl"></div>
                                            <div class="h-40 bg-surface-container rounded-2xl"></div>
                                        </div>
                                    }
                                }>
                                    {move || {
                                        detail_data
                                            .get()
                                            .flatten()
                                            .map(|items: Vec<BookProgressItem>| {
                                                view! {
                                                    <BookProgressDetail
                                                        student_name=name.clone()
                                                        items=items
                                                    />
                                                }
                                                    .into_any()
                                            })
                                    }}
                                </Suspense>
                            </Sheet>
                        }
                    })
            }}
        </div>
    }
}

#[component]
fn AddMemberForm(
    class_id: i64,
    /// Kelas tujuan berkategori KBM? Menentukan apakah santri yang sudah punya
    /// kelas KBM harus DIPINDAH, bukan sekadar ditambah.
    kelas_kbm: bool,
    #[prop(into)] nama_kelas: String,
    refetch: impl Fn() + Copy + Send + 'static,
) -> impl IntoView {
    let q = RwSignal::new(String::new());
    // 0 = semua angkatan.
    let angkatan = RwSignal::new(0_i32);
    let angkatan_ada = Resource::new(|| (), |_| async move { angkatan_tersedia_data().await });
    let results = RwSignal::new(Vec::<StudentSearchItem>::new());
    let selected = RwSignal::new(Vec::<i64>::new());
    let busy = RwSignal::new(false);
    let msg = RwSignal::new(Option::<(bool, String)>::None);
    // Nama kelas dipakai di dalam closure konfirmasi.
    let nama_kelas = StoredValue::new(nama_kelas);
    let toggle = move |id: i64| {
        selected.update(|v| {
            if let Some(pos) = v.iter().position(|&x| x == id) {
                v.remove(pos);
            } else {
                v.push(id);
            }
        });
    };

    // Id yang SEDANG TAMPIL (hasil pencarian saat ini).
    let id_tampil = move || results.get().iter().map(|s| s.id).collect::<Vec<i64>>();
    // Semua yang tampil sudah tercentang? Menentukan tombolnya jadi "centang
    // semua" atau "hapus centang".
    let semua_tampil_tercentang = move || {
        let tampil = id_tampil();
        !tampil.is_empty() && {
            let dipilih = selected.get();
            tampil.iter().all(|id| dipilih.contains(id))
        }
    };

    // Centang/hapus-centang SELURUH hasil yang sedang tampil.
    //
    // Bekerja pada yang TAMPIL, bukan mengganti seluruh pilihan: pengelola
    // sering mencari "2024", centang semua, lalu mencari "2025" dan centang
    // semua lagi. Kalau tombol ini menimpa daftar pilihan, kelompok pertama
    // hilang tanpa ada yang memberitahu — dan baru ketahuan setelah menekan
    // Tambah. Jadi "centang semua" MENAMBAHKAN yang tampil, dan "hapus
    // centang" hanya membuang yang tampil; pilihan dari pencarian lain tetap
    // utuh.
    let toggle_semua = move |_| {
        let tampil = id_tampil();
        if tampil.is_empty() {
            return;
        }
        let hapus = semua_tampil_tercentang();
        selected.update(|v| {
            if hapus {
                v.retain(|id| !tampil.contains(id));
            } else {
                for id in tampil {
                    if !v.contains(&id) {
                        v.push(id);
                    }
                }
            }
        });
    };

    // Selalu memanggil server (query pendek/kosong → daftar default beberapa
    // santri), agar daftar tampil tanpa harus mengetik.
    let do_search = move || {
        let query = q.get_untracked();
        let ang = angkatan.get_untracked();
        leptos::task::spawn_local(async move {
            // class_id → server mengecualikan santri yang sudah di kelas ini.
            // Tanpa kata kunci & angkatan, server hanya mengirim 10 nama —
            // lihat `service::kelas::search_students`.
            if let Ok(r) = staff_search_students(query, class_id, ang).await {
                results.set(r);
            }
        });
    };

    // Muat daftar default begitu form dirender.
    Effect::new(move |prev: Option<()>| {
        if prev.is_none() {
            do_search();
        }
    });

    // Santri TERPILIH yang sudah punya kelas KBM lain — merekalah yang akan
    // DIPINDAH, bukan sekadar ditambah. Dihitung dari hasil yang tampil; yang
    // dipilih dari pencarian lain tak lagi ada di `results`, jadi angka ini
    // bisa lebih kecil dari kenyataan — server tetap yang menegakkan aturannya.
    let perlu_pindah = move || -> Vec<(String, String)> {
        if !kelas_kbm {
            return Vec::new();
        }
        let dipilih = selected.get();
        results
            .get()
            .into_iter()
            .filter(|s| dipilih.contains(&s.id))
            .filter_map(|s| s.kbm_class.clone().map(|k| (s.name.clone(), k)))
            .collect()
    };

    let add_selected = move |_| {
        if busy.get_untracked() {
            return;
        }
        let ids = selected.get_untracked();
        if ids.is_empty() {
            return;
        }
        let pindah = perlu_pindah();
        let pindahkan = !pindah.is_empty();

        // KONFIRMASI. Menambah santri ke kelas mengubah kewajiban absensinya;
        // MEMINDAHKAN antar kelas KBM mengubah wali kelas, rute perizinan, dan
        // rapornya sekaligus. Keduanya tak boleh terjadi dari satu klik yang
        // tak disengaja — apalagi setelah "centang semua".
        #[cfg(target_arch = "wasm32")]
        {
            let n = ids.len();
            let kelas = nama_kelas.get_value();
            let pesan = if pindahkan {
                let rincian = pindah
                    .iter()
                    .take(5)
                    .map(|(nama, lama)| format!("• {nama} — sekarang di \"{lama}\""))
                    .collect::<Vec<_>>()
                    .join("\n");
                let sisa = pindah.len().saturating_sub(5);
                let ekor = if sisa > 0 { format!("\n… dan {sisa} lainnya") } else { String::new() };
                format!(
                    "Tambahkan {n} santri ke \"{kelas}\"?\n\n                     {} di antaranya SUDAH punya kelas KBM dan akan DIPINDAHKAN                      (dikeluarkan dari kelas lamanya):\n{rincian}{ekor}\n\n                     Memindahkan kelas KBM mengubah wali kelas, rute perizinan, dan rapornya.",
                    pindah.len()
                )
            } else {
                format!("Tambahkan {n} santri ke kelas \"{kelas}\"?")
            };
            let ok = web_sys::window()
                .and_then(|w| w.confirm_with_message(&pesan).ok())
                .unwrap_or(false);
            if !ok {
                return;
            }
        }
        let _ = (&pindah, &nama_kelas);
        busy.set(true);
        msg.set(None);
        leptos::task::spawn_local(async move {
            match add_members_action(class_id, ids, pindahkan).await {
                Ok(n) => {
                    msg.set(Some((true, format!("{n} santri ditambahkan ke kelas."))));
                    selected.set(Vec::new());
                    q.set(String::new());
                    refetch();
                    do_search(); // refresh daftar → yg baru ditambah hilang dari pilihan
                }
                Err(e) => {
                    msg.set(Some((false, crate::web::components::pesan_galat(e))));
                }
            }
            busy.set(false);
        });
    };

    view! {
        <div class="ppm-card p-4 space-y-3">
            <div class="flex items-center gap-2">
                <span class="material-symbols-outlined text-primary">"person_add"</span>
                <h3 class="text-body-md font-bold text-on-background">"Tambah Santri"</h3>
            </div>

            // Cari santri
            <div class="relative">
                <span class="material-symbols-outlined absolute left-3 top-1/2 -translate-y-1/2 text-outline text-[20px]">
                    "search"
                </span>
                <input
                    type="text"
                    class="w-full pl-10 pr-3 py-2.5 bg-surface-container border-0 rounded-xl text-body-sm text-on-surface"
                    placeholder="Cari nama atau NIS santri…"
                    prop:value=move || q.get()
                    on:input=move |ev| {
                        q.set(event_target_value(&ev));
                        do_search();
                    }
                />
            </div>

            // Penyaring angkatan — bersama pencarian, inilah yang membuka
            // seluruh daftar (tanpa keduanya server hanya mengirim 10).
            <Suspense fallback=|| ()>
                {move || {
                    let tahun = angkatan_ada.get().and_then(|r| r.ok()).unwrap_or_default();
                    view! {
                        <select
                            class="w-full bg-surface-container border-0 rounded-xl px-3 py-2.5 text-body-sm text-on-surface"
                            on:change=move |ev| {
                                angkatan.set(event_target_value(&ev).parse::<i32>().unwrap_or(0));
                                do_search();
                            }
                        >
                            <option value="0">"Semua angkatan"</option>
                            {tahun
                                .into_iter()
                                .map(|t| {
                                    view! {
                                        <option value=t.to_string()>{format!("Angkatan {t}")}</option>
                                    }
                                })
                                .collect_view()}
                        </select>
                    }
                }}
            </Suspense>

            {move || {
                msg.get()
                    .map(|(ok, text)| {
                        let cls = if ok {
                            "p-2.5 bg-secondary-container text-on-secondary-container rounded-lg text-body-sm anim-in"
                        } else {
                            "p-2.5 bg-error-container text-on-error-container rounded-lg text-body-sm anim-in"
                        };
                        view! { <div class=cls>{text}</div> }
                    })
            }}

            {move || {
                let list = results.get();
                (!list.is_empty())
                    .then(|| {
                        let jml_tampil = list.len();
                        // DUA batas berbeda, dan keduanya harus dikatakan —
                        // "centang semua" hanya mencakup yang tampil.
                        //   • belum menyaring → server cuma mengirim 10 nama;
                        //   • sudah menyaring tapi mentok di 100.
                        let belum_disaring =
                            q.get().trim().is_empty() && angkatan.get() == 0;
                        let awal_saja = belum_disaring && jml_tampil >= 10;
                        let mentok = !belum_disaring && jml_tampil >= 100;
                        view! {
                            <div class="flex items-center justify-between gap-2">
                                <p class="text-[11px] text-on-surface-variant min-w-0">
                                    {move || {
                                        let n = selected.get().len();
                                        if n == 0 {
                                            "Centang santri (boleh banyak), lalu tekan Tambah.".to_string()
                                        } else {
                                            format!("{n} santri dipilih")
                                        }
                                    }}
                                </p>
                                <button
                                    class="px-3 py-1.5 rounded-lg border border-outline-variant text-[11px] font-semibold text-on-surface hover:border-primary hover:text-primary transition-colors cursor-pointer shrink-0 whitespace-nowrap"
                                    on:click=toggle_semua
                                >
                                    {move || {
                                        if semua_tampil_tercentang() {
                                            format!("Hapus centang ({jml_tampil})")
                                        } else {
                                            format!("Centang semua ({jml_tampil})")
                                        }
                                    }}
                                </button>
                            </div>
                            {awal_saja
                                .then(|| {
                                    view! {
                                        <p class="text-[11px] text-on-surface-variant">
                                            "Menampilkan 10 santri pertama. Cari nama/NIS atau pilih angkatan untuk melihat semuanya."
                                        </p>
                                    }
                                })}
                            {mentok
                                .then(|| {
                                    view! {
                                        <p class="text-[11px] text-warning">
                                            "Menampilkan 100 teratas — persempit pencarian bila santri yang dicari belum muncul."
                                        </p>
                                    }
                                })}
                            <div class="space-y-2">
                                {list
                                    .into_iter()
                                    .map(|s| {
                                        let id = s.id;
                                        // Segmen kosong tak ikut tercetak — kelas "-" pada
                                        // santri tanpa kelas hanya jadi tanda hubung
                                        // menggantung.
                                        // Angkatan ikut ditampilkan: begitu daftarnya
                                        // disaring per angkatan, itulah yang dipakai
                                        // pengelola untuk memastikan saringannya benar.
                                        let mut bagian = vec![format!("NIS: {}", s.nis)];
                                        if let Some(t) = s.entry_year {
                                            bagian.push(format!("Angkatan {t}"));
                                        }
                                        match s.class_name.trim() {
                                            "" | "-" => {}
                                            k => bagian.push(k.to_string()),
                                        }
                                        let meta = bagian.join(" • ");
                                        // Sudah punya kelas KBM DAN kelas tujuan juga KBM →
                                        // mencentangnya berarti MEMINDAHKAN, bukan menambah.
                                        let pindah_dari = kelas_kbm.then(|| s.kbm_class.clone()).flatten();
                                        let checked = move || selected.with(|v| v.contains(&id));
                                        view! {
                                            <label class="flex items-start gap-3 p-2.5 bg-surface-container rounded-lg anim-in cursor-pointer">
                                                <input
                                                    type="checkbox"
                                                    class="w-5 h-5 accent-primary cursor-pointer shrink-0 mt-0.5"
                                                    prop:checked=checked
                                                    on:change=move |_| toggle(id)
                                                />
                                                <div class="flex-1 min-w-0">
                                                    <p class="text-body-sm font-semibold text-on-background truncate">
                                                        {s.name}
                                                    </p>
                                                    <p class="text-[12px] text-on-surface-variant truncate">{meta}</p>
                                                    {pindah_dari
                                                        .map(|lama| {
                                                            view! {
                                                                <span class="mt-1 inline-flex items-center gap-1 px-2 py-0.5 rounded-full bg-warning/10 text-warning text-[11px] font-semibold">
                                                                    <span class="material-symbols-outlined text-[14px]">
                                                                        "swap_horiz"
                                                                    </span>
                                                                    {format!("Pindahkan dari KBM \"{lama}\"")}
                                                                </span>
                                                            }
                                                        })}
                                                </div>
                                            </label>
                                        }
                                    })
                                    .collect_view()}
                            </div>
                            <button
                                class="w-full py-2.5 rounded-xl bg-primary text-on-primary font-semibold text-body-sm cursor-pointer press disabled:opacity-60"
                                prop:disabled=move || busy.get() || selected.get().is_empty()
                                on:click=add_selected
                            >
                                {move || {
                                    let n = selected.get().len();
                                    let pindah = perlu_pindah().len();
                                    if busy.get() {
                                        "Menambahkan…".to_string()
                                    } else if n == 0 {
                                        "Pilih santri dulu".to_string()
                                    } else if pindah > 0 {
                                        format!("Tambah {n} santri ({pindah} dipindahkan)")
                                    } else {
                                        format!("Tambah {n} santri")
                                    }
                                }}
                            </button>
                        }
                    })
            }}
        </div>
    }
}
