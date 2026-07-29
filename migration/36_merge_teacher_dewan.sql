-- 36_merge_teacher_dewan.sql — Gabung role 'teacher' → 'dewan_guru' (satu peran
-- guru saja, biar tak membingungkan). Wali kelas tetap (classes.wali_kelas_id
-- menunjuk user id yang sama; hanya kolom role user yang berubah). Peran guru
-- kini melihat data se-pesantren (dewan_guru scope).

UPDATE users SET role = 'dewan_guru' WHERE role = 'teacher';
