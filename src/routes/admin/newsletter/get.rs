use actix_web::{http::header::ContentType, HttpResponse};
use actix_web_flash_messages::IncomingFlashMessages;
use std::fmt::Write;

pub async fn publish_newsletter_form(
    flash_msg: IncomingFlashMessages,
) -> Result<HttpResponse, actix_web::Error> {
    let mut html_msg = String::new();
    for m in flash_msg.iter() {
        writeln!(html_msg, "<p><i>{}</i></p>", m.content()).unwrap();
    }
    let idempotency_key = uuid::Uuid::new_v4();
    Ok(HttpResponse::Ok()
        .content_type(ContentType::html())
        .body(format!(
            r#"<!DOCTYPE html>
    <html lang="en">
    <head>
        <meta http-equiv="content-type" content="text/html; charset=utf-8">
        <title>Publish Newsletter Issue</title>
    </head>
    <body>
        {html_msg}
        <form action="/admin/newsletters" method="post">
            <label>Title:<br>
                <input
                    type="text"
                    placeholder="Enter the issue title"
                    name="title"
                >
            </label>
            <br>
            <label>Plain text content:<br>
                <textarea
                    placeholder="Enter the content in plain text"
                    name="text_content"
                    rows="20"
                    cols="50"
                ></textarea>
            </label>
            <br>
            <label>HTML content:<br>
                <textarea
                    placeholder="Enter the content in HTML format"
                    name="html_content"
                    rows="20"
                    cols="50"
                ></textarea>
            </label>
            <br>
            <input hidden type="text" name="idempotency_key" value="{idempotency_key}">
            <button type="submit">Publish</button>
        </form>
        <p><a href="/admin/dashboard">&lt;- Back</a></p>
    </body>
    </html>"#
        )))
}
