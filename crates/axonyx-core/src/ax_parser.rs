use thiserror::Error;

use crate::ax_ast::prelude::*;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum AxParseError {
    #[error("document is empty")]
    EmptyDocument,
    #[error("tabs are not supported in indentation at line {line}")]
    TabsNotSupported { line: usize },
    #[error("indentation must use multiples of two spaces at line {line}")]
    InvalidIndentation { line: usize },
    #[error("expected `page <Name>` at line {line}")]
    InvalidPage { line: usize },
    #[error("unexpected indentation at line {line}")]
    UnexpectedIndentation { line: usize },
    #[error("invalid data binding at line {line}")]
    InvalidDataBinding { line: usize },
    #[error("invalid each statement at line {line}")]
    InvalidEach { line: usize },
    #[error("invalid pipeline stage at line {line}")]
    InvalidPipelineStage { line: usize },
    #[error("invalid component syntax at line {line}")]
    InvalidComponent { line: usize },
    #[error("invalid expression at line {line}: {message}")]
    InvalidExpression { line: usize, message: String },
}

#[derive(Debug, Clone)]
struct AxLine {
    line: usize,
    indent: usize,
    text: String,
}

pub fn parse_ax(input: &str) -> Result<AxDocument, AxParseError> {
    let lines = preprocess(input)?;
    if lines.is_empty() {
        return Err(AxParseError::EmptyDocument);
    }

    let mut parser = Parser { lines, pos: 0 };
    parser.parse_document()
}

struct Parser {
    lines: Vec<AxLine>,
    pos: usize,
}

impl Parser {
    fn parse_document(&mut self) -> Result<AxDocument, AxParseError> {
        let page_line = self.current().ok_or(AxParseError::EmptyDocument)?.clone();
        if page_line.indent != 0 || !page_line.text.starts_with("page ") {
            return Err(AxParseError::InvalidPage {
                line: page_line.line,
            });
        }

        let name = page_line.text["page ".len()..].trim();
        if name.is_empty() {
            return Err(AxParseError::InvalidPage {
                line: page_line.line,
            });
        }

        self.pos += 1;
        let body = self.parse_block(2)?;

        Ok(AxDocument {
            page: AxPage::new(name, body),
        })
    }

    fn parse_block(&mut self, indent: usize) -> Result<Vec<AxStatement>, AxParseError> {
        let mut statements = Vec::new();

        while let Some(line) = self.current() {
            if line.indent < indent {
                break;
            }

            if line.indent > indent {
                return Err(AxParseError::UnexpectedIndentation { line: line.line });
            }

            statements.push(self.parse_statement(indent)?);
        }

        Ok(statements)
    }

    fn parse_statement(&mut self, indent: usize) -> Result<AxStatement, AxParseError> {
        let line = self.current().expect("checked by parse_block").clone();

        if let Some(next) = self.peek(1) {
            if next.indent == indent + 2 && next.text.starts_with("|> ") {
                return self.parse_pipeline(indent);
            }
        }

        if line.text.starts_with("data ") {
            self.parse_data()
        } else if line.text.starts_with("each ") {
            self.parse_each(indent)
        } else {
            self.parse_component(indent)
        }
    }

    fn parse_data(&mut self) -> Result<AxStatement, AxParseError> {
        let line = self.current().expect("line exists").clone();
        let body = line.text["data ".len()..].trim();
        let Some((name, expr)) = body.split_once('=') else {
            return Err(AxParseError::InvalidDataBinding { line: line.line });
        };

        let name = name.trim();
        if name.is_empty() {
            return Err(AxParseError::InvalidDataBinding { line: line.line });
        }

        let value = parse_expr(expr.trim(), line.line)?;
        self.pos += 1;

        Ok(AxStatement::data(name, value))
    }

    fn parse_each(&mut self, indent: usize) -> Result<AxStatement, AxParseError> {
        let line = self.current().expect("line exists").clone();
        let body = line.text["each ".len()..].trim();
        let Some((binding, source)) = body.split_once(" in ") else {
            return Err(AxParseError::InvalidEach { line: line.line });
        };

        let binding = binding.trim();
        if binding.is_empty() {
            return Err(AxParseError::InvalidEach { line: line.line });
        }

        let source = parse_expr(source.trim(), line.line)?;
        self.pos += 1;
        let body = self.parse_block(indent + 2)?;

        Ok(AxStatement::each(binding, source, body))
    }

    fn parse_component(&mut self, indent: usize) -> Result<AxStatement, AxParseError> {
        let line = self.current().expect("line exists").clone();
        let component = parse_component_line(&line.text, line.line)?;
        self.pos += 1;

        let component = match component.body {
            AxBody::Empty => {
                if let Some(next) = self.current() {
                    if next.indent == indent + 2 && !next.text.starts_with("|> ") {
                        let body = self.parse_block(indent + 2)?;
                        component.block(body)
                    } else {
                        component
                    }
                } else {
                    component
                }
            }
            _ => component,
        };

        Ok(AxStatement::component(component))
    }

    fn parse_pipeline(&mut self, indent: usize) -> Result<AxStatement, AxParseError> {
        let line = self.current().expect("line exists").clone();
        let source = parse_expr(&line.text, line.line)?;
        self.pos += 1;

        let mut pipeline = AxPipeline::new(source);

        while let Some(stage_line) = self.current() {
            if stage_line.indent < indent + 2 {
                break;
            }

            if stage_line.indent != indent + 2 || !stage_line.text.starts_with("|> ") {
                return Err(AxParseError::InvalidPipelineStage {
                    line: stage_line.line,
                });
            }

            let stage_text = stage_line.text["|> ".len()..].trim();
            if let Some(binding) = stage_text.strip_prefix("Each ") {
                let binding = binding.trim();
                if binding.is_empty() {
                    return Err(AxParseError::InvalidPipelineStage {
                        line: stage_line.line,
                    });
                }
                pipeline = pipeline.stage(AxPipelineStage::Each(AxEachStage::new(binding)));
            } else {
                pipeline = pipeline.stage(AxPipelineStage::Component(parse_component_line(
                    stage_text,
                    stage_line.line,
                )?));
            }

            self.pos += 1;
        }

        Ok(AxStatement::pipeline(pipeline))
    }

    fn current(&self) -> Option<&AxLine> {
        self.lines.get(self.pos)
    }

    fn peek(&self, offset: usize) -> Option<&AxLine> {
        self.lines.get(self.pos + offset)
    }
}

fn preprocess(input: &str) -> Result<Vec<AxLine>, AxParseError> {
    let mut lines = Vec::new();

    for (index, raw) in input.lines().enumerate() {
        let line_no = index + 1;
        if raw.trim().is_empty() {
            continue;
        }

        if raw.contains('\t') {
            return Err(AxParseError::TabsNotSupported { line: line_no });
        }

        let indent = raw.chars().take_while(|c| *c == ' ').count();
        if indent % 2 != 0 {
            return Err(AxParseError::InvalidIndentation { line: line_no });
        }

        lines.push(AxLine {
            line: line_no,
            indent,
            text: raw.trim().to_string(),
        });
    }

    Ok(lines)
}

fn parse_component_line(input: &str, line: usize) -> Result<AxComponent, AxParseError> {
    let (head, inline) = split_inline_arrow(input);
    let (name, rest) = split_first_token(head).ok_or(AxParseError::InvalidComponent { line })?;

    if name.is_empty() || !is_component_name(name) {
        return Err(AxParseError::InvalidComponent { line });
    }

    let mut component = AxComponent::new(name);

    if !rest.trim().is_empty() {
        for part in split_top_level(rest.trim(), ',') {
            let Some((prop_name, value)) = part.split_once(':') else {
                return Err(AxParseError::InvalidComponent { line });
            };

            let prop_name = prop_name.trim();
            let value = parse_expr(value.trim(), line)?;

            match prop_name {
                "recipe" => component = component.recipe(value),
                "class" => component = component.class(value),
                _ => component = component.prop(prop_name, value),
            }
        }
    }

    if let Some(inline_expr) = inline {
        component = component.inline(parse_expr(inline_expr.trim(), line)?);
    }

    Ok(component)
}

fn is_component_name(name: &str) -> bool {
    let mut chars = name.chars();
    let Some(first) = chars.next() else {
        return false;
    };

    if !first.is_ascii_alphabetic() {
        return false;
    }

    if first.is_ascii_uppercase() {
        return chars.all(|ch| ch.is_ascii_alphanumeric());
    }

    chars.all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '-')
}

fn split_inline_arrow(input: &str) -> (&str, Option<&str>) {
    if let Some(index) = find_top_level(input, "->") {
        let head = input[..index].trim_end();
        let tail = input[index + 2..].trim_start();
        (head, Some(tail))
    } else {
        (input, None)
    }
}

fn split_first_token(input: &str) -> Option<(&str, &str)> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return None;
    }

    if let Some(index) = trimmed.find(char::is_whitespace) {
        Some((&trimmed[..index], trimmed[index..].trim_start()))
    } else {
        Some((trimmed, ""))
    }
}

fn parse_expr(input: &str, line: usize) -> Result<AxExpr, AxParseError> {
    let input = input.trim();
    if input.is_empty() {
        return Err(AxParseError::InvalidExpression {
            line,
            message: "expression is empty".to_string(),
        });
    }

    if (input.starts_with('"') && input.ends_with('"'))
        || (input.starts_with('\'') && input.ends_with('\''))
    {
        return Ok(AxExpr::string(input[1..input.len() - 1].to_string()));
    }

    if input == "true" {
        return Ok(AxExpr::bool(true));
    }
    if input == "false" {
        return Ok(AxExpr::bool(false));
    }

    if let Ok(value) = input.parse::<i64>() {
        return Ok(AxExpr::number(value));
    }

    if input.ends_with(')') {
        if let Some(open_index) = find_call_open(input) {
            let path = input[..open_index].trim();
            let args = &input[open_index + 1..input.len() - 1];
            let path: Vec<String> = path
                .split('.')
                .map(str::trim)
                .filter(|segment| !segment.is_empty())
                .map(ToOwned::to_owned)
                .collect();

            if path.is_empty() {
                return Err(AxParseError::InvalidExpression {
                    line,
                    message: format!("invalid call path `{input}`"),
                });
            }

            let args = if args.trim().is_empty() {
                Vec::new()
            } else {
                split_top_level(args, ',')
                    .into_iter()
                    .map(|part| parse_expr(part.trim(), line))
                    .collect::<Result<Vec<_>, _>>()?
            };

            return Ok(AxExpr::Call { path, args });
        }
    }

    if input.contains('.') {
        let mut parts = input.split('.').map(str::trim);
        let first = parts.next().unwrap_or_default();
        if first.is_empty() {
            return Err(AxParseError::InvalidExpression {
                line,
                message: format!("invalid member expression `{input}`"),
            });
        }

        let mut expr = AxExpr::ident(first);
        for property in parts {
            if property.is_empty() {
                return Err(AxParseError::InvalidExpression {
                    line,
                    message: format!("invalid member expression `{input}`"),
                });
            }
            expr = expr.member(property);
        }
        return Ok(expr);
    }

    Ok(AxExpr::ident(input))
}

fn find_call_open(input: &str) -> Option<usize> {
    let mut in_string: Option<char> = None;

    for (index, ch) in input.char_indices() {
        match in_string {
            Some(quote) => {
                if ch == quote {
                    in_string = None;
                }
            }
            None => match ch {
                '"' | '\'' => in_string = Some(ch),
                '(' => return Some(index),
                _ => {}
            },
        }
    }

    None
}

fn split_top_level(input: &str, delimiter: char) -> Vec<&str> {
    let mut result = Vec::new();
    let mut start = 0usize;
    let mut depth = 0usize;
    let mut in_string: Option<char> = None;

    for (index, ch) in input.char_indices() {
        match in_string {
            Some(quote) => {
                if ch == quote {
                    in_string = None;
                }
            }
            None => match ch {
                '"' | '\'' => in_string = Some(ch),
                '(' => depth += 1,
                ')' => depth = depth.saturating_sub(1),
                _ if ch == delimiter && depth == 0 => {
                    result.push(input[start..index].trim());
                    start = index + ch.len_utf8();
                }
                _ => {}
            },
        }
    }

    result.push(input[start..].trim());
    result
}

fn find_top_level(input: &str, needle: &str) -> Option<usize> {
    let mut depth = 0usize;
    let mut in_string: Option<char> = None;
    let chars: Vec<(usize, char)> = input.char_indices().collect();
    let needle_chars: Vec<char> = needle.chars().collect();

    let mut i = 0usize;
    while i < chars.len() {
        let (byte_index, ch) = chars[i];
        match in_string {
            Some(quote) => {
                if ch == quote {
                    in_string = None;
                }
            }
            None => match ch {
                '"' | '\'' => in_string = Some(ch),
                '(' => depth += 1,
                ')' => depth = depth.saturating_sub(1),
                _ => {
                    if depth == 0 && matches_needle(&chars, i, &needle_chars) {
                        return Some(byte_index);
                    }
                }
            },
        }
        i += 1;
    }

    None
}

fn matches_needle(chars: &[(usize, char)], start: usize, needle: &[char]) -> bool {
    if start + needle.len() > chars.len() {
        return false;
    }

    chars[start..start + needle.len()]
        .iter()
        .map(|(_, ch)| *ch)
        .eq(needle.iter().copied())
}

pub mod prelude {
    pub use super::parse_ax;
    pub use super::AxParseError;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_indentation_first_page() {
        let input = r#"
page Home
  data posts = Db.Stream("posts")

  Container max: "xl"
    Grid cols: 3, gap: "md"
      each post in posts
        Card title: post.title
          Copy -> post.excerpt
"#;

        let document = parse_ax(input).expect("document should parse");

        assert_eq!(document.page.name, "Home");
        assert_eq!(document.page.body.len(), 2);
    }

    #[test]
    fn parses_component_style_layers() {
        let input = r#"
page Home
  Button tone: "primary", size: "lg", recipe: "hero-cta", class: "w-full" -> "Launch"
"#;

        let document = parse_ax(input).expect("document should parse");

        let AxStatement::Component(button) = &document.page.body[0] else {
            panic!("expected button component");
        };

        assert_eq!(button.props.len(), 2);
        assert_eq!(button.style.recipe, Some(AxExpr::string("hero-cta")));
        assert_eq!(button.style.class, Some(AxExpr::string("w-full")));
    }

    #[test]
    fn parses_native_html_tag_component() {
        let input = r#"
page Home
  section class: "hero-shell"
    a href: "/docs", target: "_blank" -> "Read docs"
"#;

        let document = parse_ax(input).expect("document should parse");

        let AxStatement::Component(section) = &document.page.body[0] else {
            panic!("expected section component");
        };

        assert_eq!(section.name, "section");
        assert_eq!(section.style.class, Some(AxExpr::string("hero-shell")));

        let AxBody::Block(body) = &section.body else {
            panic!("expected block body");
        };

        let AxStatement::Component(anchor) = &body[0] else {
            panic!("expected anchor component");
        };

        assert_eq!(anchor.name, "a");
        assert_eq!(anchor.props.len(), 2);
    }

    #[test]
    fn parses_pipeline_sketch() {
        let input = r#"
page Home
  Db.Stream("users")
    |> Grid cols: 2
    |> Each user
    |> ProfileCard
"#;

        let document = parse_ax(input).expect("document should parse");

        let AxStatement::Pipeline(pipeline) = &document.page.body[0] else {
            panic!("expected pipeline");
        };

        assert_eq!(pipeline.stages.len(), 3);
    }
}
