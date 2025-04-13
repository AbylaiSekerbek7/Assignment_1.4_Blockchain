use actix_web::{web, App, HttpServer, Responder, HttpResponse};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::env;
use dotenv::dotenv;
use serde_json::json;
use vader_sentiment::SentimentIntensityAnalyzer;

#[derive(Deserialize)]
struct NewsQuery {
    symbol: String,
}

#[derive(Debug, Deserialize, Serialize)]
struct News {
    title: String,
    description: String,
    date: String,
    url: Option<String>,
    source: String,
    image_url: Option<String>,
    sentiment: String,
}

// Sentiment analysis of news articles
fn analyze_sentiment(text: &str) -> String {
    let analyzer = SentimentIntensityAnalyzer::new();
    let score = analyzer.polarity_scores(text);

    if score["compound"] >= 0.05 {
        "🟢 Positive".to_string()
    } else if score["compound"] <= -0.05 {
        "🔴 Negative".to_string()
    } else {
        "🟡 Neutral".to_string()
    }
}

// CoinMarketCap API
async fn get_coinmarketcap_news(query: &str) -> Result<Vec<News>, reqwest::Error> {
    let api_key = "63e211b3-5923-4625-8ba4-afdaa10cba16";
    let url = format!(
        "https://pro-api.coinmarketcap.com/v1/cryptocurrency/info?symbol={}",
        query
    );

    let client = Client::new();
    let res = client
        .get(&url)
        .header("X-CMC_PRO_API_KEY", api_key)
        .send()
        .await?;

    let json: serde_json::Value = res.json().await?;
    println!("🔍 CoinMarketCap response:\n{:#}", json);

    let mut news = Vec::new();
    if let Some(articles) = json["data"].as_object() {
        for (_symbol, article) in articles {
            if let Some(name) = article["name"].as_str() {
                let title = name;
                let description = article["description"].as_str().unwrap_or("No description");
                let sentiment = analyze_sentiment(&format!("{} {}", title, description));

                let news_item = News {
                    title: title.to_string(),
                    description: description.to_string(),
                    date: article["date_added"].as_str().unwrap_or("Unknown date").to_string(),
                    url: Some(article["urls"]["website"][0].as_str().unwrap_or("#").to_string()),
                    source: "CoinMarketCap".to_string(),
                    image_url: Some(article["logo"].as_str().unwrap_or("").to_string()),
                    sentiment,
                };

                news.push(news_item);
            }
        }
    }

    Ok(news)
}

// CoinDesk API
async fn get_coindesk_news(category: &str) -> Result<Vec<News>, reqwest::Error> {
    let url = format!(
        "https://data-api.coindesk.com/news/v1/article/list?lang=EN&limit=10&categories={}",
        category
    );

    let client = Client::new();
    let res = client
        .get(&url)
        .header("Content-Type", "application/json; charset=UTF-8")
        .send()
        .await?;

    let json: serde_json::Value = res.json().await?;
    println!("📰 CoinDesk response:\n{:#}", json);

    let mut news = Vec::new();
    if let Some(articles) = json["Data"].as_array() {
        for article in articles {
            let title = article["TITLE"].as_str().unwrap_or("No Title");
            let description = article["BODY"].as_str().unwrap_or("No Summary");
            let date = article["CREATED_ON"].to_string();
            let url = article["GUID"].as_str().unwrap_or("#");
            let image_url = article["IMAGE_URL"].as_str().unwrap_or("");

            let sentiment = analyze_sentiment(&format!("{} {}", title, description));

            news.push(News {
                title: title.to_string(),
                description: description.to_string(),
                date,
                url: Some(url.to_string()),
                source: "CoinDesk".to_string(),
                image_url: Some(image_url.to_string()),
                sentiment,
            });        
        }
    }

    Ok(news)
}

// Основная функция для обработки запроса новостей
async fn get_news(query: web::Query<NewsQuery>) -> impl Responder {
    let coinmarketcap_news = get_coinmarketcap_news(&query.symbol).await;
    let coindesk_news = get_coindesk_news(&query.symbol).await;

    match (coinmarketcap_news, coindesk_news) {
        (Ok(mut cmc), Ok(mut desk)) => {
            cmc.append(&mut desk);
            HttpResponse::Ok().json(cmc)
        }
        (Ok(cmc), Err(_)) => HttpResponse::Ok().json(cmc),
        (Err(_), Ok(desk)) => HttpResponse::Ok().json(desk),
        _ => HttpResponse::InternalServerError().body("Error fetching news"),
    }
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    dotenv().ok();
    println!("🚀 Server running at http://localhost:8080");

    HttpServer::new(|| {
        App::new()
            .route("/news", web::get().to(get_news))
            .service(actix_files::Files::new("/", "./static").show_files_listing())
    })
    .bind("127.0.0.1:8080")?
    .run()
    .await
}