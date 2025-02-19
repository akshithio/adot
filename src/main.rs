use chrono::Utc;
use clap::{Command, arg, ArgMatches};
use firestore::*;
use serde::{Deserialize, Serialize};
use std::env;
use uuid::Uuid;
use reqwest;
#[derive(Debug, Clone, Deserialize, Serialize)]
struct MicroblogStruct {
    id: String,
    content: String,
    time: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct TimeStruct {
    utc: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct LocationStruct {
    city: String,
    region: String,
    country: String,
    timezone: String,
    time: TimeStruct,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let matches = Command::new("adot")
        .version("1.0")
        .author("Akshith Garapati")
        .about("CLI tool for microblogging and location tracking")
        .subcommand(
            Command::new("microblog")
                .about("Create a new microblog post")
                .arg(arg!([content] "The content of the microblog post").required(true)),
        )
        .subcommand(
            Command::new("location")
                .about("Sends your current location to Firestore"),
        )
        .get_matches();

    if let Some(sub_matches) = matches.subcommand_matches("microblog") {
        handle_microblog(sub_matches).await?;
    } else if matches.subcommand_matches("location").is_some() {
        handle_location().await?;
    } else {
        println!("No valid subcommand provided. Use `adot microblog 'your content'` or `adot location`.");
    }
    Ok(())
}

async fn handle_microblog(matches: &ArgMatches) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let content = matches.get_one::<String>("content").unwrap();
    
    let id = Uuid::new_v4().to_string();
    let timestamp = Utc::now().to_rfc3339();

    let project_id = env::var("PROJECT_ID").map_err(|e| format!("PROJECT_ID not found: {}", e))?;
    let google_credentials = env::var("GOOGLE_APPLICATION_CREDENTIALS")
        .map_err(|e| format!("GOOGLE_APPLICATION_CREDENTIALS not found: {}", e))?;

    unsafe {
        std::env::set_var("GOOGLE_APPLICATION_CREDENTIALS", google_credentials);
    }

    let db = FirestoreDb::new(&project_id).await?;
    const COLLECTION_NAME: &str = "microblog";

    let microblog_struct = MicroblogStruct {
        id,
        content: content.to_string(),
        time: timestamp,
    };

    let object_returned: MicroblogStruct = db
        .fluent()
        .insert()
        .into(COLLECTION_NAME)
        .document_id(&microblog_struct.id)
        .object(&microblog_struct)
        .execute()
        .await?;

    println!("Inserted: {:?}", object_returned);

    Ok(())
}

async fn handle_location() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let timestamp = Utc::now().to_rfc3339();


    let project_id = env::var("PROJECT_ID").map_err(|e| format!("PROJECT_ID not found: {}", e))?;
    let google_credentials = env::var("GOOGLE_APPLICATION_CREDENTIALS")
        .map_err(|e| format!("GOOGLE_APPLICATION_CREDENTIALS not found: {}", e))?;
    let ipinfo_token = env::var("IPINFO_TOKEN")
        .map_err(|e| format!("IPINFO_TOKEN not found: {}", e))?;

    unsafe {
        std::env::set_var("GOOGLE_APPLICATION_CREDENTIALS", google_credentials);
    }

    let db = FirestoreDb::new(&project_id).await?;
    const COLLECTION_NAME: &str = "location";

    println!("🗑️  Cleaning up existing location entry...");
    if let Ok(_) = db.fluent()
        .delete()
        .from(COLLECTION_NAME)
        .document_id("latest")
        .execute()
        .await 

    {
        println!("Deleted existing 'latest' entry");
    }

    println!("📍 Fetching location data from ipinfo.io...");
    let url = format!("https://ipinfo.io/json?token={}", ipinfo_token);
    
    let response = reqwest::get(&url).await?;
    let status = response.status();
    
    if !status.is_success() {
        let error_text = response.text().await?;
        return Err(format!("API request failed with status {}: {}", status, error_text).into());
    }

    let location_data: serde_json::Value = response.json().await?;
    let location_struct = LocationStruct {
        city: location_data["city"].as_str()
            .ok_or("Missing city field")?.to_string(),
        region: location_data["region"].as_str()
            .ok_or("Missing region field")?.to_string(),
        country: location_data["country"].as_str()
            .ok_or("Missing country field")?.to_string(),
        timezone: location_data["timezone"].as_str()
            .ok_or("Missing timezone field")?.to_string(),
        time: TimeStruct {
            utc: timestamp,
        },
    };

    let object_returned: LocationStruct = db
        .fluent()
        .insert()
        .into(COLLECTION_NAME)
        .document_id("latest")
        .object(&location_struct)
        .execute()
        .await?;

    println!("✅ Updated location: {:?}", object_returned);
    Ok(())
}