use rocket::response::content::RawHtml;
use rust_embed::Embed;

#[derive(Embed)]
#[folder = "src/docs"]
struct Docs;

#[get("/")]
pub(crate) fn scalar_docs_ui() -> RawHtml<String>
{
    RawHtml(
        std::str::from_utf8(
            Docs::get("docs.html")
                .expect("Docs HTML file must be included in the binary")
                .data.as_ref()
        )
            .expect("Docs HTML file must be valid utf-8")
            .into()
    )
}