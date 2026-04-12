use thiserror::Error;

use crate::ax_ast::prelude::AxExpr;
use crate::ax_backend_ast::prelude::*;
use crate::ax_query_ast::prelude::*;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum AxBackendParseError {
    #[error("document is empty")]
    EmptyDocument,
    #[error("tabs are not supported in indentation at line {line}")]
    TabsNotSupported { line: usize },
    #[error("indentation must use multiples of two spaces at line {line}")]
    InvalidIndentation { line: usize },
    #[error("unexpected indentation at line {line}")]
    UnexpectedIndentation { line: usize },
    #[error("invalid block header at line {line}")]
    InvalidBlock { line: usize },
    #[error("invalid data binding at line {line}")]
    InvalidDataBinding { line: usize },
    #[error("invalid input section at line {line}")]
    InvalidInputSection { line: usize },
    #[error("invalid field declaration at line {line}")]
    InvalidField { line: usize },
    #[error("invalid mutation at line {line}")]
    InvalidMutation { line: usize },
    #[error("invalid assignment at line {line}")]
    InvalidAssignment { line: usize },
    #[error("invalid return statement at line {line}")]
    InvalidReturn { line: usize },
    #[error("invalid send statement at line {line}")]
    InvalidSend { line: usize },
    #[error("invalid query source at line {line}")]
    InvalidQuerySource { line: usize },
    #[error("invalid query clause at line {line}")]
    InvalidQueryClause { line: usize },
    #[error("invalid query number at line {line}")]
    InvalidQueryNumber { line: usize },
    #[error("invalid expression at line {line}: {message}")]
    InvalidExpression { line: usize, message: String },
}

#[derive(Debug, Clone)]
struct BackendLine {
    line: usize,
    indent: usize,
    text: String,
}

pub fn parse_backend_ax(input: &str) -> Result<AxBackendDocument, AxBackendParseError> {
    let lines = preprocess(input)?;
    if lines.is_empty() {
        return Err(AxBackendParseError::EmptyDocument);
    }

    let mut parser = Parser { lines, pos: 0 };
    parser.parse_document()
}

struct Parser {
    lines: Vec<BackendLine>,
    pos: usize,
}

impl Parser {
    fn parse_document(&mut self) -> Result<AxBackendDocument, AxBackendParseError> {
        let mut blocks = Vec::new();

        while self.current().is_some() {
            let line = self.current().expect("checked");
            if line.indent != 0 {
                return Err(AxBackendParseError::UnexpectedIndentation { line: line.line });
            }
            blocks.push(self.parse_block()?);
        }

        Ok(AxBackendDocument::new(blocks))
    }

    fn parse_block(&mut self) -> Result<AxBackendBlock, AxBackendParseError> {
        let line = self.current().expect("block line exists").clone();
        let text = line.text.as_str();

        if let Some(rest) = text.strip_prefix("route ") {
            let mut parts = rest.splitn(2, ' ');
            let method = parts.next().unwrap_or_default().trim();
            let path = parts.next().unwrap_or_default().trim();
            if method.is_empty() || path.is_empty() {
                return Err(AxBackendParseError::InvalidBlock { line: line.line });
            }

            self.pos += 1;
            let body = self.parse_body(2)?;
            return Ok(AxBackendBlock::Route(AxRoute::new(
                method,
                trim_quotes(path),
                body,
            )));
        }

        if let Some(name) = text.strip_prefix("loader ") {
            let name = name.trim();
            if name.is_empty() {
                return Err(AxBackendParseError::InvalidBlock { line: line.line });
            }

            self.pos += 1;
            let body = self.parse_body(2)?;
            return Ok(AxBackendBlock::Loader(AxLoader::new(name, body)));
        }

        if let Some(name) = text.strip_prefix("action ") {
            let name = name.trim();
            if name.is_empty() {
                return Err(AxBackendParseError::InvalidBlock { line: line.line });
            }

            self.pos += 1;
            let (input, body) = self.parse_action_sections(2)?;
            return Ok(AxBackendBlock::Action(
                AxAction::new(name).input(input).body(body),
            ));
        }

        if let Some(name) = text.strip_prefix("job ") {
            let name = name.trim();
            if name.is_empty() {
                return Err(AxBackendParseError::InvalidBlock { line: line.line });
            }

            self.pos += 1;
            let body = self.parse_body(2)?;
            return Ok(AxBackendBlock::Job(AxJob::new(name, body)));
        }

        Err(AxBackendParseError::InvalidBlock { line: line.line })
    }

    fn parse_action_sections(
        &mut self,
        indent: usize,
    ) -> Result<(Vec<AxField>, Vec<AxBackendStmt>), AxBackendParseError> {
        let mut input = Vec::new();
        let mut body = Vec::new();

        while let Some(line) = self.current() {
            if line.indent < indent {
                break;
            }

            if line.indent > indent {
                return Err(AxBackendParseError::UnexpectedIndentation { line: line.line });
            }

            if line.text == "input:" {
                self.pos += 1;
                input = self.parse_input_fields(indent + 2)?;
            } else {
                body.push(self.parse_statement(indent)?);
            }
        }

        Ok((input, body))
    }

    fn parse_input_fields(&mut self, indent: usize) -> Result<Vec<AxField>, AxBackendParseError> {
        let mut fields = Vec::new();

        while let Some(line) = self.current() {
            if line.indent < indent {
                break;
            }

            if line.indent > indent {
                return Err(AxBackendParseError::UnexpectedIndentation { line: line.line });
            }

            let Some((name, ty)) = line.text.split_once(':') else {
                return Err(AxBackendParseError::InvalidField { line: line.line });
            };

            let name = name.trim();
            let ty = ty.trim();
            if name.is_empty() || ty.is_empty() {
                return Err(AxBackendParseError::InvalidField { line: line.line });
            }

            fields.push(AxField::new(name, ty));
            self.pos += 1;
        }

        if fields.is_empty() {
            let line = self
                .current()
                .map(|line| line.line)
                .unwrap_or(self.lines.last().map(|line| line.line).unwrap_or(1));
            return Err(AxBackendParseError::InvalidInputSection { line });
        }

        Ok(fields)
    }

    fn parse_body(&mut self, indent: usize) -> Result<Vec<AxBackendStmt>, AxBackendParseError> {
        let mut body = Vec::new();

        while let Some(line) = self.current() {
            if line.indent < indent {
                break;
            }

            if line.indent > indent {
                return Err(AxBackendParseError::UnexpectedIndentation { line: line.line });
            }

            body.push(self.parse_statement(indent)?);
        }

        Ok(body)
    }

    fn parse_statement(&mut self, indent: usize) -> Result<AxBackendStmt, AxBackendParseError> {
        let line = self.current().expect("statement line exists").clone();
        let text = line.text.as_str();

        if text.starts_with("data ") {
            return self.parse_data();
        }

        if text.starts_with("insert ") {
            return self.parse_mutation(indent, true);
        }

        if text.starts_with("update ") {
            return self.parse_mutation(indent, false);
        }

        if text.starts_with("delete ") {
            return self.parse_delete(indent);
        }

        if let Some(value) = text.strip_prefix("revalidate ") {
            self.pos += 1;
            return Ok(AxBackendStmt::revalidate(parse_expr(
                value.trim(),
                line.line,
            )?));
        }

        if let Some(value) = text.strip_prefix("return ") {
            self.pos += 1;
            let value = value.trim();
            if value.is_empty() {
                return Err(AxBackendParseError::InvalidReturn { line: line.line });
            }
            if value == "ok" {
                return Ok(AxBackendStmt::r#return("ok"));
            }
            return Ok(AxBackendStmt::r#return(parse_expr(value, line.line)?));
        }

        if let Some(rest) = text.strip_prefix("send ") {
            let Some((target, payload)) = rest.split_once(" with ") else {
                return Err(AxBackendParseError::InvalidSend { line: line.line });
            };

            let target = target.trim();
            let payload = payload.trim();
            if target.is_empty() || payload.is_empty() {
                return Err(AxBackendParseError::InvalidSend { line: line.line });
            }

            self.pos += 1;
            return Ok(AxBackendStmt::send(target, parse_expr(payload, line.line)?));
        }

        Err(AxBackendParseError::InvalidBlock { line: line.line })
    }

    fn parse_data(&mut self) -> Result<AxBackendStmt, AxBackendParseError> {
        let line = self.current().expect("data line exists").clone();
        let body = line.text["data ".len()..].trim();
        let Some((name, expr)) = body.split_once('=') else {
            return Err(AxBackendParseError::InvalidDataBinding { line: line.line });
        };

        let name = name.trim();
        let expr = expr.trim();
        if name.is_empty() || expr.is_empty() {
            return Err(AxBackendParseError::InvalidDataBinding { line: line.line });
        }

        let expr = parse_expr(expr, line.line)?;
        self.pos += 1;

        if let Some(next) = self.current() {
            if next.indent == line.indent + 2 && is_query_clause(&next.text) {
                let query = self.parse_query_spec(expr, line.line, line.indent + 2)?;
                return Ok(AxBackendStmt::data(name, query));
            }
        }

        if let Ok(source) = query_source_from_expr(expr.clone(), line.line) {
            return Ok(AxBackendStmt::data(name, AxQuerySpec::new(source)));
        }

        Ok(AxBackendStmt::data(name, expr))
    }

    fn parse_mutation(
        &mut self,
        indent: usize,
        is_insert: bool,
    ) -> Result<AxBackendStmt, AxBackendParseError> {
        let line = self.current().expect("mutation line exists").clone();
        let prefix = if is_insert { "insert " } else { "update " };
        let collection = line.text[prefix.len()..].trim();
        if collection.is_empty() {
            return Err(AxBackendParseError::InvalidMutation { line: line.line });
        }

        self.pos += 1;
        let (fields, filters) = self.parse_mutation_body(indent + 2)?;

        if is_insert {
            let mut mutation = AxMutation::new(trim_quotes(collection), fields);
            for filter in filters {
                mutation = mutation.filter(filter);
            }
            Ok(AxBackendStmt::Insert(mutation))
        } else {
            let mut mutation = AxMutation::new(trim_quotes(collection), fields);
            for filter in filters {
                mutation = mutation.filter(filter);
            }
            Ok(AxBackendStmt::Update(mutation))
        }
    }

    fn parse_delete(&mut self, indent: usize) -> Result<AxBackendStmt, AxBackendParseError> {
        let line = self.current().expect("delete line exists").clone();
        let collection = line.text["delete ".len()..].trim();
        if collection.is_empty() {
            return Err(AxBackendParseError::InvalidMutation { line: line.line });
        }

        self.pos += 1;
        let (_fields, filters) = self.parse_mutation_body(indent + 2)?;
        let mut mutation = AxMutation::new(trim_quotes(collection), []);
        for filter in filters {
            mutation = mutation.filter(filter);
        }
        Ok(AxBackendStmt::Delete(mutation))
    }

    fn parse_mutation_body(
        &mut self,
        indent: usize,
    ) -> Result<(Vec<AxAssignment>, Vec<AxQueryFilter>), AxBackendParseError> {
        let mut fields = Vec::new();
        let mut filters = Vec::new();
        let mut parsing_filters = false;

        while let Some(line) = self.current() {
            if line.indent < indent {
                break;
            }

            if line.indent > indent {
                return Err(AxBackendParseError::UnexpectedIndentation { line: line.line });
            }

            if let Some(rest) = line.text.strip_prefix("where ") {
                parsing_filters = true;
                let Some((field, value)) = rest.split_once('=') else {
                    return Err(AxBackendParseError::InvalidQueryClause { line: line.line });
                };

                let field = field.trim();
                let value = value.trim();
                if field.is_empty() || value.is_empty() {
                    return Err(AxBackendParseError::InvalidQueryClause { line: line.line });
                }

                filters.push(AxQueryFilter::new(
                    field,
                    AxQueryFilterOp::Eq,
                    parse_expr(value, line.line)?,
                ));
                self.pos += 1;
                continue;
            }

            if parsing_filters {
                return Err(AxBackendParseError::InvalidQueryClause { line: line.line });
            }

            let Some((name, value)) = line.text.split_once(':') else {
                return Err(AxBackendParseError::InvalidAssignment { line: line.line });
            };

            let name = name.trim();
            let value = value.trim();
            if name.is_empty() || value.is_empty() {
                return Err(AxBackendParseError::InvalidAssignment { line: line.line });
            }

            fields.push(AxAssignment::new(name, parse_expr(value, line.line)?));
            self.pos += 1;
        }

        if fields.is_empty() && filters.is_empty() {
            let line = self
                .current()
                .map(|line| line.line)
                .unwrap_or(self.lines.last().map(|line| line.line).unwrap_or(1));
            return Err(AxBackendParseError::InvalidMutation { line });
        }

        Ok((fields, filters))
    }

    fn current(&self) -> Option<&BackendLine> {
        self.lines.get(self.pos)
    }

    fn parse_query_spec(
        &mut self,
        expr: AxExpr,
        line: usize,
        indent: usize,
    ) -> Result<AxQuerySpec, AxBackendParseError> {
        let source = query_source_from_expr(expr, line)?;
        let mut query = AxQuerySpec::new(source);

        while let Some(clause_line) = self.current() {
            if clause_line.indent < indent {
                break;
            }

            if clause_line.indent != indent {
                return Err(AxBackendParseError::UnexpectedIndentation {
                    line: clause_line.line,
                });
            }

            let text = clause_line.text.as_str();
            if let Some(rest) = text.strip_prefix("where ") {
                let Some((field, value)) = rest.split_once('=') else {
                    return Err(AxBackendParseError::InvalidQueryClause {
                        line: clause_line.line,
                    });
                };

                let field = field.trim();
                let value = value.trim();
                if field.is_empty() || value.is_empty() {
                    return Err(AxBackendParseError::InvalidQueryClause {
                        line: clause_line.line,
                    });
                }

                query = query.filter(AxQueryFilter::new(
                    field,
                    AxQueryFilterOp::Eq,
                    parse_expr(value, clause_line.line)?,
                ));
                self.pos += 1;
                continue;
            }

            if let Some(rest) = text.strip_prefix("order ") {
                let mut parts = rest.split_whitespace();
                let field = parts.next().unwrap_or_default();
                if field.is_empty() {
                    return Err(AxBackendParseError::InvalidQueryClause {
                        line: clause_line.line,
                    });
                }

                let direction = match parts.next() {
                    Some(value) if value.eq_ignore_ascii_case("desc") => {
                        AxQueryOrderDirection::Desc
                    }
                    Some(value) if value.eq_ignore_ascii_case("asc") => AxQueryOrderDirection::Asc,
                    None => AxQueryOrderDirection::Asc,
                    Some(_) => {
                        return Err(AxBackendParseError::InvalidQueryClause {
                            line: clause_line.line,
                        })
                    }
                };

                query = query.order(AxQueryOrder::new(field, direction));
                self.pos += 1;
                continue;
            }

            if let Some(rest) = text.strip_prefix("limit ") {
                let value = rest.trim().parse::<u32>().map_err(|_| {
                    AxBackendParseError::InvalidQueryNumber {
                        line: clause_line.line,
                    }
                })?;
                query = query.limit(value);
                self.pos += 1;
                continue;
            }

            if let Some(rest) = text.strip_prefix("offset ") {
                let value = rest.trim().parse::<u32>().map_err(|_| {
                    AxBackendParseError::InvalidQueryNumber {
                        line: clause_line.line,
                    }
                })?;
                query = query.offset(value);
                self.pos += 1;
                continue;
            }

            return Err(AxBackendParseError::InvalidQueryClause {
                line: clause_line.line,
            });
        }

        Ok(query)
    }
}

fn preprocess(input: &str) -> Result<Vec<BackendLine>, AxBackendParseError> {
    let mut lines = Vec::new();

    for (index, raw) in input.lines().enumerate() {
        let line_no = index + 1;
        if raw.trim().is_empty() {
            continue;
        }

        if raw.contains('\t') {
            return Err(AxBackendParseError::TabsNotSupported { line: line_no });
        }

        let indent = raw.chars().take_while(|c| *c == ' ').count();
        if indent % 2 != 0 {
            return Err(AxBackendParseError::InvalidIndentation { line: line_no });
        }

        lines.push(BackendLine {
            line: line_no,
            indent,
            text: raw.trim().to_string(),
        });
    }

    Ok(lines)
}

fn is_query_clause(text: &str) -> bool {
    text.starts_with("where ")
        || text.starts_with("order ")
        || text.starts_with("limit ")
        || text.starts_with("offset ")
}

fn query_source_from_expr(expr: AxExpr, line: usize) -> Result<AxQuerySource, AxBackendParseError> {
    match expr {
        AxExpr::Call { path, args }
            if path == vec!["Db".to_string(), "Stream".to_string()] && args.len() == 1 =>
        {
            match &args[0] {
                AxExpr::String(collection) => Ok(AxQuerySource::Stream {
                    collection: collection.clone(),
                }),
                _ => Err(AxBackendParseError::InvalidQuerySource { line }),
            }
        }
        _ => Err(AxBackendParseError::InvalidQuerySource { line }),
    }
}

fn parse_expr(input: &str, line: usize) -> Result<AxExpr, AxBackendParseError> {
    let input = input.trim();
    if input.is_empty() {
        return Err(AxBackendParseError::InvalidExpression {
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
                return Err(AxBackendParseError::InvalidExpression {
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
            return Err(AxBackendParseError::InvalidExpression {
                line,
                message: format!("invalid member expression `{input}`"),
            });
        }

        let mut expr = AxExpr::ident(first);
        for property in parts {
            if property.is_empty() {
                return Err(AxBackendParseError::InvalidExpression {
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

fn trim_quotes(input: &str) -> String {
    input
        .trim()
        .trim_matches('"')
        .trim_matches('\'')
        .to_string()
}

pub mod prelude {
    pub use super::parse_backend_ax;
    pub use super::AxBackendParseError;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_loader_and_route_blocks() {
        let input = r#"
loader PostsList
  data posts = Db.Stream("posts")
    where status = "published"
    order created_at desc
    limit 20
  return posts

route GET "/api/posts"
  data posts = Db.Stream("posts")
  return posts
"#;

        let document = parse_backend_ax(input).expect("document should parse");

        assert_eq!(document.blocks.len(), 2);
        let AxBackendBlock::Loader(loader) = &document.blocks[0] else {
            panic!("expected loader block");
        };
        assert_eq!(loader.name, "PostsList");
        let AxBackendStmt::Data(posts) = &loader.body[0] else {
            panic!("expected data statement");
        };
        assert_eq!(
            posts.value,
            AxBackendValue::Query(
                AxQuerySpec::new(AxQuerySource::Stream {
                    collection: "posts".to_string(),
                })
                .filter(AxQueryFilter::new(
                    "status",
                    AxQueryFilterOp::Eq,
                    AxExpr::string("published"),
                ))
                .order(AxQueryOrder::new("created_at", AxQueryOrderDirection::Desc,))
                .limit(20)
            )
        );

        let AxBackendBlock::Route(route) = &document.blocks[1] else {
            panic!("expected route block");
        };
        assert_eq!(route.method, "GET");
        assert_eq!(route.path, "/api/posts");
    }

    #[test]
    fn parses_plain_stream_binding_as_query_without_extra_clauses() {
        let input = r#"
loader PostsList
  data posts = Db.Stream("posts")
  return posts
"#;

        let document = parse_backend_ax(input).expect("document should parse");

        let AxBackendBlock::Loader(loader) = &document.blocks[0] else {
            panic!("expected loader block");
        };
        let AxBackendStmt::Data(posts) = &loader.body[0] else {
            panic!("expected data statement");
        };

        assert_eq!(
            posts.value,
            AxBackendValue::Query(AxQuerySpec::new(AxQuerySource::Stream {
                collection: "posts".to_string(),
            }))
        );
    }

    #[test]
    fn parses_action_with_input_and_mutations() {
        let input = r#"
action CreatePost
  input:
    title: string
    excerpt: string

  insert "posts"
    title: input.title
    excerpt: input.excerpt

  revalidate "/posts"
  return ok
"#;

        let document = parse_backend_ax(input).expect("document should parse");

        let AxBackendBlock::Action(action) = &document.blocks[0] else {
            panic!("expected action block");
        };

        assert_eq!(action.name, "CreatePost");
        assert_eq!(action.input.len(), 2);
        assert_eq!(action.body.len(), 3);
    }

    #[test]
    fn parses_update_mutation_with_where_clause() {
        let input = r#"
action PublishPost
  input:
    id: i64
    title: string

  update "posts"
    title: input.title
    where id = input.id

  return ok
"#;

        let document = parse_backend_ax(input).expect("document should parse");

        let AxBackendBlock::Action(action) = &document.blocks[0] else {
            panic!("expected action block");
        };

        let AxBackendStmt::Update(mutation) = &action.body[0] else {
            panic!("expected update statement");
        };

        assert_eq!(mutation.collection, "posts");
        assert_eq!(mutation.fields.len(), 1);
        assert_eq!(mutation.filters.len(), 1);
        assert_eq!(mutation.filters[0].field, "id");
    }

    #[test]
    fn parses_job_send_step() {
        let input = r#"
job PublishDailyDigest
  data posts = Query.PublishedPosts()
  send DigestEmail with posts
"#;

        let document = parse_backend_ax(input).expect("document should parse");

        let AxBackendBlock::Job(job) = &document.blocks[0] else {
            panic!("expected job block");
        };

        assert_eq!(job.name, "PublishDailyDigest");
        assert_eq!(job.body.len(), 2);
    }

    #[test]
    fn parses_delete_mutation_with_where_clause() {
        let input = r#"
action RemovePost
  input:
    id: i64

  delete "posts"
    where id = input.id

  return ok
"#;

        let document = parse_backend_ax(input).expect("document should parse");

        let AxBackendBlock::Action(action) = &document.blocks[0] else {
            panic!("expected action block");
        };

        let AxBackendStmt::Delete(mutation) = &action.body[0] else {
            panic!("expected delete statement");
        };

        assert_eq!(mutation.collection, "posts");
        assert!(mutation.fields.is_empty());
        assert_eq!(mutation.filters.len(), 1);
    }
}
