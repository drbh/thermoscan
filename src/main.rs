use btleplug::{
    api::{Central, CentralEvent, Manager as _, ScanFilter},
    platform::{Adapter, Manager},
};
use dotenv::dotenv;
use dotenv_codegen::dotenv;
use futures::stream::StreamExt;
use reqwest::header;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::error::Error;

// Assuming the manufacturer data layout has a fixed structure.
const GOVEE_ID: &str = "454c4c495f52";

// Read in secrets for loki from .env file
const LOKI_TOKEN: &str = dotenv!("LOKI_TOKEN");
const LOKI_STREAM_VALUE: &str = dotenv!("LOKI_STREAM_VALUE");
const LOKI_URL: &str = dotenv!("LOKI_URL");

// Always just use the first adapter found (our rpi only has one).
async fn get_first_central(manager: &Manager) -> Option<Adapter> {
    manager.adapters().await.ok()?.into_iter().next()
}

// SensorReading is the data we want to send to loki.
// We'll construct this from the advertisement data emitted by the sensor.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct SensorReading {
    id: String,
    temperature: f32,
    battery: f32,
    humidity: f32,
    timestamp: u64,
    mac: String,
}

// A simple HTTP POST to loki.
async fn send_log(
    url: &str,
    token: &str,
    stream_value: &str,
    sensor_reading: &SensorReading,
) -> Result<(), Box<dyn std::error::Error>> {
    let timestamp = sensor_reading.timestamp * 1_000_000_000;
    let json_body = json!({
        "id": sensor_reading.id,
        "temperature": sensor_reading.temperature,
        "battery": sensor_reading.battery,
        "humidity": sensor_reading.humidity,
        "mac": sensor_reading.mac,
    });

    let mut headers = header::HeaderMap::new();
    headers.insert("Authorization", format!("Basic {token}").parse().unwrap());
    headers.insert("Content-Type", "application/json".parse().unwrap());
    headers.insert("User-Agent", "thermoscan/1.0.0".parse().unwrap());

    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .unwrap();
    let res = client
        .post(url)
        .headers(headers)
        .body(
            json!({
                "streams": [
                    {
                        "stream": {
                            "house": stream_value
                        },
                        "values": [
                            [
                                format!("{}", timestamp),
                                json_body.to_string()
                            ]
                        ]
                    }
                ]
            })
            .to_string(),
        )
        .send()
        .await?
        .text()
        .await?;
    println!("{}", res);

    Ok(())
}

impl SensorReading {
    fn from_data(id: &str, data: &[u8]) -> Option<Self> {
        Some(Self {
            id: id.to_string(),
            temperature: get_temp(data),
            battery: get_battery(data),
            humidity: get_humidity(data),
            timestamp: get_timestamp(),
            mac: get_mac(data),
        })
    }
}

// The mac is the last 6 bytes of the manufacturer data.
fn get_mac(data: &[u8]) -> String {
    hex::encode(data.get(5..11).unwrap())
}

// The temperature is the first 3 bytes of the manufacturer data.
fn get_temp(data: &[u8]) -> f32 {
    u32::from_str_radix(&hex::encode(data.get(1..4).unwrap()), 16).unwrap() as f32 / 10_000.0
}

// The battery is the 4th byte of the manufacturer data.
fn get_battery(data: &[u8]) -> f32 {
    u32::from_str_radix(&hex::encode(data.get(4..5).unwrap()), 16).unwrap() as f32 / 10.0
}

// The humidity is the last 3 bytes of the temperature.
fn get_humidity(data: &[u8]) -> f32 {
    get_temp(data) * 10_000.0 % 1_000.0 / 10.0
}

// The timestamp is the current time in seconds.
fn get_timestamp() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

// Whever we get an event, we'll try to parse it into a SensorReading.
// If we can, we'll send it to loki.
fn handle_event(event: CentralEvent) -> Option<SensorReading> {
    if let CentralEvent::ManufacturerDataAdvertisement {
        id,
        manufacturer_data,
    } = event
    {
        let id_str = id.to_string();
        if let Some((_, data)) = manufacturer_data.clone().into_iter().next() {
            if let Some(sensor_reading) = SensorReading::from_data(&id_str, &data) {
                if let Some(mac_data) = manufacturer_data.get(&60552) {
                    let mac = get_mac(mac_data);
                    if mac == GOVEE_ID {
                        return Some(sensor_reading);
                    }
                    return None;
                } else {
                    return None;
                }
            }
        }
    }

    None
}

// This app will run forever, scanning for bluetooth advertisements. Whenever it sees
// an advertisement from a Govee sensor, it will parse the data and send it to loki.
#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    dotenv().ok();

    println!("Starting Bluetooth scanner");
    pretty_env_logger::init();

    let manager = Manager::new().await?;
    let central = get_first_central(&manager)
        .await
        .ok_or("No adapters found")?;
    let mut events = central.events().await?;

    central.start_scan(ScanFilter::default()).await?;

    while let Some(event) = events.next().await {
        if let Some(sensor_reading) = handle_event(event) {
            if let Err(e) = send_log(LOKI_URL, LOKI_TOKEN, LOKI_STREAM_VALUE, &sensor_reading).await
            {
                println!("Error sending log: {}", e);
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_mac() {
        let data = vec![
            0, 10, 100, 255, 100, 100, // mac
            0, 0, 0, 0, 0, 0, 0,
        ];
        let mac = get_mac(&data);
        assert_eq!(mac, "640000000000");
    }

    #[test]
    fn test_get_temp() {
        let mac_data = vec![0, 10, 100, 255];
        let temp = get_temp(&mac_data);
        assert_eq!(temp, 68.1215);
    }

    #[test]
    fn test_get_humidity() {
        let mac_data = vec![0, 10, 100, 255];
        let humidity = get_humidity(&mac_data);
        assert_eq!(humidity, 21.5);
    }

    #[test]
    fn test_get_battery() {
        let mac_data = vec![0, 0, 0, 0, 100, 100];
        let battery = get_battery(&mac_data);
        assert_eq!(battery, 10.0);
    }

    #[test]
    fn test_from_manufacturer_data() {
        let id = "1234";
        let data = vec![
            0, 10, 100, 255, 100, 100, // mac
            0, 0, 0, 0, 0, 0, 0,
        ];
        let sensor_reading = SensorReading::from_data(id, &data).unwrap();
        assert_eq!(sensor_reading.id, "1234");
        assert_eq!(sensor_reading.temperature, 68.1215);
        assert_eq!(sensor_reading.battery, 10.0);
        assert_eq!(sensor_reading.humidity, 21.5);
        assert_eq!(sensor_reading.mac, "640000000000");
    }
}
