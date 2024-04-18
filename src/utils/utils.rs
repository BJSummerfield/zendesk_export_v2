pub struct Utils;

impl Utils {
    pub fn sanitize_name(name: &str) -> String {
        name.chars()
            .filter(|c| c.is_alphanumeric() || *c == ' ' || *c == '-' || *c == '_')
            .map(|c| if c == ' ' { '_' } else { c })
            .collect()
    }

    // get rid of cat and tag
    // pub fn convert_html_to_markdown(html: &str, title: &str) -> String {
    //     let markdown_content = html2md::parse_html(html);
    //
    //     let front_matter = Self::create_front_matter(title);
    //
    //     format!("{}{}", front_matter, markdown_content)
    // }

    pub fn create_front_matter(title: &str) -> String {
        return format!("---\ntitle: \"{}\"\n---\n\n", title);
    }
}
