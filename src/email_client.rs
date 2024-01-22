use crate::domain::SubscriberEmail;

pub struct EmailClient {
    http_client: reqwest::Client,
    base_url: String,
    sender: SubscriberEmail,
}

impl EmailClient {
    pub fn new(base_url: String, sender: SubscriberEmail) -> Self {
        Self {
            sender,
            http_client: reqwest::Client::new(),
            base_url,
        }
    }
}

impl EmailClient {
    pub async fn send_email(
        &self,
        recipient: SubscriberEmail,
        subject: &str,
        html_content: &str,
        text_content: &str
    ) -> Result<(), String> {
        todo!()
    }
}

#[cfg(test)]
mod tests {
    use crate::domain::SubscriberEmail;
    use crate::email_client::EmailClient;
    use fake::faker::{internet::en::SafeEmail, lorem::en::{Paragraph, Sentence}};
    use fake::{Fake, Faker};
    use wiremock::matchers::any;
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn send_email_fires_request_to_base_url() {
        let mock_server = MockServer::start().await;
        let sender = SubscriberEmail::parse_email(SafeEmail().fake()).unwrap();
        let email_client = EmailClient::new(mock_server.uri(), sender);

        Mock::given(any())
            .respond_with(ResponseTemplate::new(200))
            .expect(1)
            .mount(&mock_server)
            .await;
        let subscriber_email = SubscriberEmail::parse_email(SafeEmail().fake()).unwrap();
        let subject = Sentence(1..2).fake::<String>();
        let content = Paragraph(1..10).fake::<String>();

        let _ = email_client
            .send_email(subscriber_email, &subject, &content, &content)
            .await;
    }
}