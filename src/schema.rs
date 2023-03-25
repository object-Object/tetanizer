use lazy_static::lazy_static;
use regex::Regex;
use serenity::model::prelude::*;
use strum::{Display, EnumIter, IntoEnumIterator};
use tantivy::{doc, schema::*, DateTime};

fn is_mime_type(attachment: &Attachment, mime_type: &str) -> bool {
    match &attachment.content_type {
        Some(t) => t.starts_with(mime_type),
        None => false,
    }
}

fn has_mime_type(message: &Message, mime_type: &str) -> bool {
    message
        .attachments
        .iter()
        .any(|a| is_mime_type(a, mime_type))
}

#[derive(Display, EnumIter)]
#[strum(serialize_all = "snake_case")]
pub enum MessageMediaType {
    Link,
    Embed,
    File,
    Video,
    Image,
    Sound,
    Sticker,
}

impl MessageMediaType {
    pub fn is_in_message(&self, message: &Message) -> bool {
        lazy_static! {
            static ref LINK_RE: Regex = Regex::new(r"https?://").unwrap();
        }
        match self {
            Self::Link => LINK_RE.is_match(&message.content),
            Self::Embed => !message.embeds.is_empty(),
            Self::File => !message.attachments.is_empty(),
            Self::Video => has_mime_type(message, "video"),
            Self::Image => has_mime_type(message, "image"),
            Self::Sound => has_mime_type(message, "audio"),
            Self::Sticker => !message.sticker_items.is_empty(),
        }
    }
}

pub struct MessageSchema {
    /// message id
    id: Field,
    author_id: Field,
    channel_id: Field,

    /// main message text
    content: Field,
    timestamp: Field,
    /// if the message is pinned
    pinned: Field,

    /// all the text in any embeds on the message (may be multi-valued)
    embed_content: Field,
    /// ids of any users mention (may be multi-valued)
    mention_user_id: Field,
    /// ids of any roles mention (may be multi-valued)
    mention_role_id: Field,
    /// media type(s) in the message (link, image, etc; may be multi-valued)
    has: Field,

    inner: Schema,
}

impl MessageSchema {
    pub fn build() -> Self {
        let mut schema_builder = Schema::builder();

        Self {
            // never need to search by message id, you can just use a message link for that
            id: schema_builder.add_u64_field("id", STORED),
            author_id: schema_builder.add_u64_field("author_id", INDEXED),
            // fast so we can filter by the user's permissions
            // also need this for putting a channel mention in the output message
            channel_id: schema_builder.add_u64_field("channel_id", INDEXED | FAST),

            // stored so we can display it in the output message
            content: schema_builder.add_text_field("content", TEXT | STORED),
            // don't need fast here to do unbounded ranges
            // for before, make the start 0
            // for after, make the end the current timestamp
            timestamp: schema_builder.add_date_field("timestamp", INDEXED),
            pinned: schema_builder.add_bool_field("pinned", INDEXED),

            embed_content: schema_builder.add_text_field("embed_content", TEXT),
            mention_user_id: schema_builder.add_u64_field("mention_user_id", INDEXED),
            mention_role_id: schema_builder.add_u64_field("mention_role_id", INDEXED),
            // string instead of text because this comes from the MessageMediaType enum
            // it's always a single word so no need to tokenize it
            has: schema_builder.add_text_field("has", STRING),

            // build the schema LAST
            inner: schema_builder.build(),
        }
    }

    pub fn parse_message(&self, message: &Message) -> Document {
        // set fields that don't need to be done in a loop here
        let mut doc = doc!(
            self.id => message.id.0,
            self.author_id => message.author.id.0,
            self.channel_id => message.channel_id.0,

            self.content => message.content.clone(),
            self.timestamp => DateTime::from_timestamp_secs(message.timestamp.unix_timestamp()),
            self.pinned => message.pinned,
        );

        for embed in &message.embeds {
            // TODO: make this less nasty
            self.add_embed_content(&mut doc, embed.author.as_ref().map(|a| &a.name));
            self.add_embed_content(&mut doc, embed.description.as_ref());
            for field in &embed.fields {
                self.add_embed_content(&mut doc, Some(&field.name));
                self.add_embed_content(&mut doc, Some(&field.value));
            }
            self.add_embed_content(&mut doc, embed.footer.as_ref().map(|f| &f.text));
            self.add_embed_content(&mut doc, embed.title.as_ref());
        }

        for user in &message.mentions {
            doc.add_u64(self.mention_user_id, user.id.0);
        }

        for role_id in &message.mention_roles {
            doc.add_u64(self.mention_role_id, role_id.0);
        }

        for media_type in MessageMediaType::iter() {
            if media_type.is_in_message(message) {
                doc.add_text(self.has, media_type.to_string());
            }
        }

        doc
    }

    pub fn inner(&self) -> &Schema {
        &self.inner
    }

    pub fn clone_inner(&self) -> Schema {
        self.inner.clone()
    }

    fn add_embed_content(&self, doc: &mut Document, content_opt: Option<&String>) {
        if let Some(content) = content_opt {
            doc.add_text(self.embed_content, content.clone());
        }
    }
}
