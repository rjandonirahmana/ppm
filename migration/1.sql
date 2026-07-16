CREATE TABLE users (
    id              BIGSERIAL PRIMARY KEY,
    
    rfid_cards      BIGINT UNIQUE NULL,
    password_hash   TEXT NOT NULL,

    full_name       VARCHAR(200) NOT NULL,

    phone_number    VARCHAR(20),

    profile_photo    VARCHAR(200) DEFAULT NULL,
    address          TEXT DEFAULT NULL,

    role            VARCHAR(20) NOT NULL CHECK (
        role IN (
            'admin',
            'teacher',
            'supervisor',
            'santri',
            'parent'
        )
    ),

    status          VARCHAR(20) NOT NULL DEFAULT 'active' CHECK (
        status IN (
            'active',
            'holiday',
            'sick',
            'suspended',
            'graduated',
            'dropped_out'
        )
    ),

    is_active       BOOLEAN NOT NULL DEFAULT TRUE,

    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);




CREATE TABLE rfid_devices (
    id              BIGSERIAL PRIMARY KEY,

    device_name     VARCHAR(100) NOT NULL,

    serial_number   VARCHAR(100) UNIQUE,

    api_key         TEXT NOT NULL,

    location        VARCHAR(200)
);




CREATE TABLE classes (
    id              BIGSERIAL PRIMARY KEY,

    name            VARCHAR(100) NOT NULL,

    description     TEXT,

    status          VARCHAR(20) NOT NULL DEFAULT 'active'
                    CHECK (
                        status IN ('active', 'inactive')
                    ),

    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);


CREATE TABLE class_schedules (
    id                  BIGSERIAL PRIMARY KEY,

    class_id            BIGINT NOT NULL
                        REFERENCES classes(id)
                        ON DELETE CASCADE,

    title               VARCHAR(100),

    start_time          TIME NOT NULL,

    end_time            TIME NOT NULL,

    limit_entery_time   TIME NOT NULL,

    recurrence_type     VARCHAR(20) NOT NULL CHECK (
        recurrence_type IN (
            'once',
            'daily',
            'weekly',
            'monthly',
            'custom'
        )
    ),

    recurrence_rule     JSONB,

    start_date          DATE NOT NULL,

    end_date            DATE,

    status              VARCHAR(20) NOT NULL DEFAULT 'active'
                        CHECK (
                            status IN ('active','inactive')
                        ),

    created_at          TIMESTAMPTZ DEFAULT NOW()
);


CREATE TABLE class_participants (
    id              BIGSERIAL PRIMARY KEY,

    class_id        BIGINT NOT NULL
                    REFERENCES classes(id)
                    ON DELETE CASCADE,


    class_schedule_id   BIGINT NOT NULL
                    REFERENCES class_schedules(id)
                    ON DELETE CASCADE,

    user_id         BIGINT NOT NULL
                    REFERENCES users(id)
                    ON DELETE CASCADE,


    entry_at       TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    UNIQUE(class_id, user_id, class_schedule_id)
);





CREATE TABLE class_sessions (
    id                  BIGSERIAL PRIMARY KEY,

    class_id            BIGINT NOT NULL
                        REFERENCES classes(id),

    class_schedule_id   BIGINT
                        REFERENCES class_schedules(id),

    teacher_id          BIGINT
                        REFERENCES users(id),

    title               VARCHAR(150),

    session_date        DATE NOT NULL,

    started_at          TIMESTAMPTZ,

    ended_at            TIMESTAMPTZ,

    status              VARCHAR(20) NOT NULL DEFAULT 'scheduled'
                        CHECK (
                            status IN (
                                'scheduled',
                                'ongoing',
                                'finished',
                                'cancelled'
                            )
                        ),

    recording_path      TEXT NULL,

    recording_duration  INTEGER,

    recording_size      BIGINT,

    recording_mime_type VARCHAR(50),

    created_at          TIMESTAMPTZ NOT NULL DEFAULT NOW()
);





CREATE TABLE class_session_chats (
    id              BIGSERIAL PRIMARY KEY,

    session_id      BIGINT NOT NULL
                    REFERENCES class_sessions(id)
                    ON DELETE CASCADE,

    user_id         BIGINT NOT NULL
                    REFERENCES users(id)
                    ON DELETE CASCADE,

    message         TEXT NOT NULL,

    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    is_deleted      BOOLEAN NOT NULL DEFAULT FALSE
);




CREATE TABLE complaints (
    id                  BIGSERIAL PRIMARY KEY,

    reporter_id         BIGINT
                        REFERENCES users(id)
                        ON DELETE SET NULL,

    reported_user_id    BIGINT
                        REFERENCES users(id)
                        ON DELETE SET NULL,

    title               VARCHAR(200) NOT NULL,

    message             TEXT NOT NULL,

    is_anonymous        BOOLEAN NOT NULL DEFAULT FALSE,

    status              VARCHAR(20) NOT NULL DEFAULT 'pending'
                        CHECK (
                            status IN (
                                'pending',
                                'in_review',
                                'resolved',
                                'rejected'
                            )
                        ),

    handled_by          BIGINT
                        REFERENCES users(id)
                        ON DELETE SET NULL,

    handled_at          TIMESTAMPTZ,


    file_path           TEXT NULL,

    mime_type           VARCHAR(100),

    file_size           BIGINT,

    response            TEXT,

    -- FIX: kolom `category` sebelumnya tergantung DI LUAR CREATE TABLE (SQL
    -- invalid → migrasi gagal jalan). Dipindah ke dalam tabel complaints.
    category            VARCHAR(30) NOT NULL DEFAULT 'other' CHECK (
        category IN (
            'discipline',
            'violence',
            'bullying',
            'facility',
            'other'
        )
    ),

    created_at          TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    updated_at          TIMESTAMPTZ NOT NULL DEFAULT NOW()
);