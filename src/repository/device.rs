//! repository/device.rs — Query tabel rfid_devices.

use anyhow::Result;
use deadpool_postgres::Pool;

pub struct DeviceRow {
    pub id: i64,
    pub device_name: String,
    pub location: Option<String>,
}

pub async fn find_device_by_key(pool: &Pool, api_key: &str) -> Result<Option<DeviceRow>> {
    let c = pool.get().await?;
    let row = c
        .query_opt(
            "SELECT id, device_name, location FROM rfid_devices WHERE api_key = $1",
            &[&api_key],
        )
        .await?;
    Ok(row.map(|r| DeviceRow {
        id: r.get(0),
        device_name: r.get(1),
        location: r.get(2),
    }))
}
