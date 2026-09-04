// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use serde_json::{Value, json};
use slint_editor_mcp::{EditorComment, ProjectComments, project_resource_uri, scan_projects};
use std::collections::BTreeMap;
use std::io::{self, BufRead, Write};
use std::path::{Path, PathBuf};

const MENTION_SEARCH_TOOL: &str = "search_visual_editor_comments";
const GET_COMMENTS_TOOL: &str = "get_visual_editor_comments";

fn main() -> io::Result<()> {
    let stdin = io::stdin();
    let mut stdout = io::stdout().lock();
    for line in stdin.lock().lines() {
        let response = handle_request(&line?);
        if let Some(response) = response {
            serde_json::to_writer(&mut stdout, &response).map_err(io::Error::other)?;
            stdout.write_all(b"\n")?;
            stdout.flush()?;
        }
    }
    Ok(())
}

fn handle_request(request_text: &str) -> Option<Value> {
    let request: Value = match serde_json::from_str::<Value>(request_text) {
        Ok(request) if request.is_object() => request,
        Ok(_) => return Some(error_response(Value::Null, -32600, "Invalid request")),
        Err(error) => {
            return Some(error_response(Value::Null, -32700, &format!("Parse error: {error}")));
        }
    };
    let id = request.get("id").cloned().unwrap_or(Value::Null);
    let is_notification = request.get("id").is_none();
    let method = request.get("method").and_then(Value::as_str).unwrap_or("");
    let result = match method {
        "initialize" => Ok(initialize_result()),
        "ping" => Ok(json!({})),
        "resources/list" => list_resources(),
        "resources/read" => read_resource(request.get("params")),
        "tools/list" => Ok(json!({ "tools": [get_comments_tool(), mention_search_tool()] })),
        "tools/call" => call_tool(request.get("params")),
        "notifications/initialized" => return None,
        _ => {
            if is_notification {
                return None;
            }
            return Some(error_response(id, -32601, &format!("Method not found: {method}")));
        }
    };
    if is_notification {
        return None;
    }
    Some(match result {
        Ok(result) => success_response(id, result),
        Err(message) => error_response(id, -32602, &message),
    })
}

fn initialize_result() -> Value {
    json!({
        "protocolVersion": "2025-06-18",
        "capabilities": {
            "resources": {},
            "tools": {}
        },
        "serverInfo": {
            "name": "slint-editor-mcp",
            "version": env!("CARGO_PKG_VERSION")
        },
        "instructions": "Attach current visual editor comments from Slint projects to the next prompt."
    })
}

fn mention_search_tool() -> Value {
    json!({
        "name": MENTION_SEARCH_TOOL,
        "title": "Slint Visual Editor",
        "description": "Find projects with comments in running Slint visual editors.",
        "inputSchema": {
            "type": "object",
            "properties": {
                "path": {
                    "type": "array",
                    "items": { "type": "string" }
                },
                "query": { "type": "string" }
            },
            "required": ["path", "query"],
            "additionalProperties": false
        },
        "_meta": {
            "openai/extensions": {
                "mentions/search": {}
            }
        }
    })
}

fn get_comments_tool() -> Value {
    json!({
        "name": GET_COMMENTS_TOOL,
        "title": "Get Slint Visual Editor Comments",
        "description": "Get comments from running Slint visual editors whose projects are within the working directory.",
        "inputSchema": {
            "type": "object",
            "properties": {
                "workingDirectory": {
                    "type": "string",
                    "description": "The absolute working directory of the current Codex task."
                }
            },
            "required": ["workingDirectory"],
            "additionalProperties": false
        },
        "annotations": {
            "readOnlyHint": true,
            "destructiveHint": false,
            "idempotentHint": true,
            "openWorldHint": false
        }
    })
}

fn list_resources() -> Result<Value, String> {
    let resources = comment_scopes()?
        .into_iter()
        .map(|project| {
            let title = project_title(&project);
            json!({
                "uri": project_resource_uri(&project.project_root),
                "name": title,
                "title": title,
                "description": format!("{} from the visual editor for {}", comment_count(project.comments.len()), project.project_root.display()),
                "mimeType": "text/markdown"
            })
        })
        .collect::<Vec<_>>();
    Ok(json!({ "resources": resources }))
}

fn read_resource(params: Option<&Value>) -> Result<Value, String> {
    let uri = params
        .and_then(|params| params.get("uri"))
        .and_then(Value::as_str)
        .ok_or_else(|| "Missing resource URI".to_string())?;
    let project = find_project(uri)?;
    Ok(json!({
        "contents": [{
            "uri": uri,
            "mimeType": "text/markdown",
            "text": format_project_comments(&project)
        }]
    }))
}

fn call_tool(params: Option<&Value>) -> Result<Value, String> {
    let params = params.ok_or_else(|| "Missing tool parameters".to_string())?;
    let name = params.get("name").and_then(Value::as_str).unwrap_or("");
    let arguments = params.get("arguments").cloned().unwrap_or_else(|| json!({}));
    match name {
        GET_COMMENTS_TOOL => Ok(tool_result(get_visual_editor_comments(&arguments))),
        MENTION_SEARCH_TOOL => Ok(tool_result(search_visual_editor_comments(&arguments))),
        _ => Err(format!("Unknown tool: {name}")),
    }
}

fn tool_result(result: Result<Value, String>) -> Value {
    result.unwrap_or_else(|message| {
        json!({
            "content": [{ "type": "text", "text": message }],
            "isError": true
        })
    })
}

fn get_visual_editor_comments(arguments: &Value) -> Result<Value, String> {
    let working_directory = parse_working_directory(arguments)?;
    let projects =
        scan_projects().map_err(|error| format!("Could not scan editor comments: {error}"))?;
    let projects = projects_beneath(&working_directory, projects);
    Ok(comments_tool_result(&working_directory, &projects))
}

fn comments_tool_result(working_directory: &Path, projects: &[ProjectComments]) -> Value {
    let comment_count = projects.iter().map(|project| project.comments.len()).sum::<usize>();
    let structured_projects = projects
        .iter()
        .map(|project| {
            json!({
                "projectRoot": project.project_root,
                "comments": project.comments.iter().map(structured_comment).collect::<Vec<_>>()
            })
        })
        .collect::<Vec<_>>();
    json!({
        "content": [{
            "type": "text",
            "text": format_directory_comments(working_directory, projects)
        }],
        "structuredContent": {
            "workingDirectory": working_directory,
            "commentCount": comment_count,
            "projects": structured_projects
        }
    })
}

fn parse_working_directory(arguments: &Value) -> Result<PathBuf, String> {
    let working_directory = arguments
        .get("workingDirectory")
        .and_then(Value::as_str)
        .ok_or_else(|| "Missing workingDirectory".to_string())?;
    let working_directory = Path::new(working_directory);
    if !working_directory.is_absolute() {
        return Err("workingDirectory must be an absolute path".into());
    }
    let working_directory = std::fs::canonicalize(working_directory)
        .map_err(|error| format!("Invalid workingDirectory: {error}"))?;
    if !working_directory.is_dir() {
        return Err("workingDirectory must be a directory".into());
    }
    Ok(working_directory)
}

fn projects_beneath(
    working_directory: &Path,
    projects: Vec<ProjectComments>,
) -> Vec<ProjectComments> {
    projects
        .into_iter()
        .filter(|project| {
            std::fs::canonicalize(&project.project_root)
                .unwrap_or_else(|_| project.project_root.clone())
                .starts_with(working_directory)
        })
        .collect()
}

fn structured_comment(comment: &EditorComment) -> Value {
    json!({
        "id": comment.id,
        "text": comment.text,
        "file": comment.file,
        "range": comment.range,
        "component": comment.component,
        "elementType": comment.element_type,
        "elementId": comment.element_id
    })
}

fn search_visual_editor_comments(arguments: &Value) -> Result<Value, String> {
    let path = arguments.get("path").and_then(Value::as_array).ok_or("Missing path")?;
    let query = arguments.get("query").and_then(Value::as_str).ok_or("Missing query")?;
    let items = if path.is_empty() { search_resource_items(query)? } else { Vec::new() };
    Ok(json!({
        "content": [{
            "type": "text",
            "text": format!("Found {} Slint visual editor comment resource(s).", items.len())
        }],
        "structuredContent": { "items": items }
    }))
}

fn search_resource_items(query: &str) -> Result<Vec<Value>, String> {
    let normalized_query = query.to_lowercase();
    Ok(comment_scopes()?
        .into_iter()
        .filter_map(|project| {
            let title = project_title(&project);
            let subtitle = format!(
                "{} · {}",
                comment_count(project.comments.len()),
                project.project_root.display()
            );
            let searchable_text = format!("{title} {subtitle}").to_lowercase();
            searchable_text.contains(&normalized_query).then(|| {
                json!({
                    "type": "resource",
                    "resourceUri": project_resource_uri(&project.project_root),
                    "title": title,
                    "subtitle": subtitle
                })
            })
        })
        .collect())
}

fn find_project(uri: &str) -> Result<ProjectComments, String> {
    comment_scopes()?
        .into_iter()
        .find(|project| project_resource_uri(&project.project_root) == uri)
        .ok_or_else(|| format!("Unknown resource URI: {uri}"))
}

fn comment_scopes() -> Result<Vec<ProjectComments>, String> {
    let projects =
        scan_projects().map_err(|error| format!("Could not scan editor comments: {error}"))?;
    Ok(expand_comment_scopes(projects))
}

fn expand_comment_scopes(projects: Vec<ProjectComments>) -> Vec<ProjectComments> {
    let mut scopes = BTreeMap::<PathBuf, Vec<EditorComment>>::new();
    for project in projects {
        let repository_root = repository_root(&project.project_root);
        let mut scope = project.project_root.as_path();
        loop {
            scopes.entry(scope.to_owned()).or_default().extend(project.comments.iter().cloned());
            if repository_root.as_deref().is_none_or(|repository_root| scope == repository_root) {
                break;
            }
            let Some(parent) = scope.parent() else {
                break;
            };
            scope = parent;
        }
    }
    scopes
        .into_iter()
        .map(|(project_root, comments)| ProjectComments { project_root, comments })
        .collect()
}

fn repository_root(project_root: &Path) -> Option<PathBuf> {
    project_root.ancestors().find(|directory| directory.join(".git").exists()).map(Path::to_owned)
}

fn project_title(project: &ProjectComments) -> String {
    let name = project
        .project_root
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
        .map(str::to_owned)
        .unwrap_or_else(|| project.project_root.to_string_lossy().into_owned());
    format!("Comments for {name} — {}", comment_count(project.comments.len()))
}

fn comment_count(count: usize) -> String {
    format!("{count} {}", if count == 1 { "comment" } else { "comments" })
}

fn format_project_comments(project: &ProjectComments) -> String {
    let mut markdown =
        format!("# Visual Editor Comments\n\nProject: `{}`\n", project.project_root.display());
    if project.comments.is_empty() {
        markdown.push_str("\nNo comments.\n");
        return markdown;
    }
    for comment in &project.comments {
        markdown.push('\n');
        markdown.push_str(&format_comment(comment, "##"));
    }
    markdown
}

fn format_directory_comments(working_directory: &Path, projects: &[ProjectComments]) -> String {
    let mut markdown = format!(
        "# Visual Editor Comments\n\nWorking directory: `{}`\n",
        working_directory.display()
    );
    if projects.is_empty() {
        markdown.push_str("\nNo matching visual editor comments are available.\n");
        return markdown;
    }
    for project in projects {
        markdown.push_str(&format!("\n## Project: `{}`\n", project.project_root.display()));
        for comment in &project.comments {
            markdown.push('\n');
            markdown.push_str(&format_comment(comment, "###"));
        }
    }
    markdown
}

fn format_comment(comment: &EditorComment, heading: &str) -> String {
    let element_id = comment
        .element_id
        .as_deref()
        .map(|element_id| format!(" #{element_id}"))
        .unwrap_or_default();
    let component = comment
        .component
        .as_deref()
        .map(|component| format!(" in `{component}`"))
        .unwrap_or_default();
    format!(
        "{heading} {}{}{}\n\n- File: `{}`\n- Range: {}:{}–{}:{}\n\n{}\n",
        comment.element_type,
        element_id,
        component,
        comment.file.display(),
        comment.range.start.line + 1,
        comment.range.start.character + 1,
        comment.range.end.line + 1,
        comment.range.end.character + 1,
        comment.text
    )
}

fn success_response(id: Value, result: Value) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "result": result })
}

fn error_response(id: Value, code: i32, message: &str) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": { "code": code, "message": message }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn comment(id: &str, file: impl Into<PathBuf>) -> EditorComment {
        EditorComment {
            id: id.into(),
            text: format!("Comment {id}"),
            file: file.into(),
            range: slint_editor_mcp::SourceRange {
                start: slint_editor_mcp::SourcePosition { line: 2, character: 4 },
                end: slint_editor_mcp::SourcePosition { line: 3, character: 8 },
            },
            component: Some("MainWindow".into()),
            element_type: "Rectangle".into(),
            element_id: Some(format!("element-{id}")),
        }
    }

    fn project(root: &str, id: &str) -> ProjectComments {
        ProjectComments {
            project_root: root.into(),
            comments: vec![comment(id, format!("{root}/main.slint"))],
        }
    }

    fn sample_projects() -> Vec<ProjectComments> {
        [
            ("/projects/slint", "root"),
            ("/projects/slint/examples/gallery", "gallery"),
            ("/projects/slint/demos/todo", "todo"),
            ("/projects/slint-sibling", "sibling"),
            ("/other/unrelated", "unrelated"),
        ]
        .into_iter()
        .map(|(root, id)| project(root, id))
        .collect()
    }

    fn project_ids(working_directory: &str) -> Vec<String> {
        projects_beneath(Path::new(working_directory), sample_projects())
            .into_iter()
            .map(|project| project.comments[0].id.clone())
            .collect()
    }

    #[test]
    fn advertises_resources_and_the_mention_search_extension() {
        let initialized =
            handle_request(r#"{"jsonrpc":"2.0","id":1,"method":"initialize"}"#).unwrap();
        assert_eq!(initialized["result"]["capabilities"]["resources"], json!({}));

        let tools = handle_request(r#"{"jsonrpc":"2.0","id":2,"method":"tools/list"}"#).unwrap();
        let tools = tools["result"]["tools"].as_array().unwrap();
        let get_comments = tools.iter().find(|tool| tool["name"] == GET_COMMENTS_TOOL).unwrap();
        assert_eq!(get_comments["inputSchema"]["required"], json!(["workingDirectory"]));
        assert_eq!(get_comments["annotations"]["readOnlyHint"], true);
        assert_eq!(get_comments["annotations"]["destructiveHint"], false);
        assert_eq!(get_comments["annotations"]["idempotentHint"], true);
        assert_eq!(get_comments["annotations"]["openWorldHint"], false);
        let mention_search = tools.iter().find(|tool| tool["name"] == MENTION_SEARCH_TOOL).unwrap();
        assert_eq!(mention_search["_meta"]["openai/extensions"]["mentions/search"], json!({}));
    }

    #[test]
    fn includes_an_exact_editor_project() {
        assert_eq!(project_ids("/projects/slint/examples/gallery"), ["gallery"]);
    }

    #[test]
    fn includes_editor_projects_beneath_a_parent_directory() {
        assert_eq!(project_ids("/projects/slint"), ["root", "gallery", "todo"]);
    }

    #[test]
    fn includes_multiple_child_editor_projects() {
        assert_eq!(project_ids("/projects/slint/examples"), ["gallery"]);
        assert_eq!(project_ids("/projects"), ["root", "gallery", "todo", "sibling"]);
    }

    #[test]
    fn excludes_sibling_editor_projects() {
        assert_eq!(project_ids("/projects/slint/demos"), ["todo"]);
    }

    #[test]
    fn excludes_unrelated_editor_projects() {
        assert!(project_ids("/workspace/unrelated").is_empty());
    }

    #[test]
    fn rejects_malformed_working_directories() {
        let temporary_directory = tempfile::tempdir().unwrap();
        let file = temporary_directory.path().join("file");
        std::fs::write(&file, []).unwrap();
        let cases = [
            (json!({}), "Missing workingDirectory"),
            (json!({ "workingDirectory": 42 }), "Missing workingDirectory"),
            (
                json!({ "workingDirectory": "relative/path" }),
                "workingDirectory must be an absolute path",
            ),
            (
                json!({ "workingDirectory": temporary_directory.path().join("missing") }),
                "Invalid workingDirectory",
            ),
            (json!({ "workingDirectory": file }), "workingDirectory must be a directory"),
        ];

        for (arguments, expected_error) in cases {
            let error = parse_working_directory(&arguments).unwrap_err();
            assert!(error.contains(expected_error), "unexpected error {error:?}");
        }
    }

    #[test]
    fn reports_a_malformed_working_directory_as_a_tool_error() {
        let response = handle_request(
            r#"{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"get_visual_editor_comments","arguments":{"workingDirectory":"relative/path"}}}"#,
        )
        .unwrap();

        assert!(response.get("error").is_none());
        assert_eq!(response["result"]["isError"], true);
        assert!(
            response["result"]["content"][0]["text"]
                .as_str()
                .unwrap()
                .contains("workingDirectory must be an absolute path")
        );
    }

    #[test]
    fn matches_a_noncanonical_published_project_root() {
        let temporary_directory = tempfile::tempdir().unwrap();
        let project_root = temporary_directory.path().join("project");
        let gallery = project_root.join("examples/gallery");
        std::fs::create_dir_all(&gallery).unwrap();
        let published_root = project_root.join("examples/../examples/gallery");
        let projects = vec![ProjectComments {
            project_root: published_root.clone(),
            comments: vec![comment("gallery", published_root.join("main.slint"))],
        }];

        let matches = projects_beneath(&std::fs::canonicalize(&project_root).unwrap(), projects);

        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].project_root, published_root);
    }

    #[test]
    fn returns_readable_markdown_and_structured_comment_metadata() {
        let projects = vec![project("/projects/slint/examples/gallery", "gallery")];
        let result = comments_tool_result(Path::new("/projects/slint"), &projects);

        assert_eq!(result["structuredContent"]["workingDirectory"], "/projects/slint");
        assert_eq!(result["structuredContent"]["commentCount"], 1);
        assert_eq!(
            result["structuredContent"]["projects"][0]["projectRoot"],
            "/projects/slint/examples/gallery"
        );
        assert_eq!(result["structuredContent"]["projects"][0]["comments"][0]["id"], "gallery");
        assert_eq!(
            result["structuredContent"]["projects"][0]["comments"][0]["elementType"],
            "Rectangle"
        );
        let markdown = result["content"][0]["text"].as_str().unwrap();
        for expected in [
            "Working directory: `/projects/slint`",
            "Project: `/projects/slint/examples/gallery`",
            "Comment gallery",
        ] {
            assert!(markdown.contains(expected), "missing {expected:?} in {markdown:?}");
        }
    }

    #[test]
    fn reports_zero_matching_comments() {
        let result = comments_tool_result(Path::new("/projects/slint"), &[]);

        assert_eq!(result["structuredContent"]["commentCount"], 0);
        assert_eq!(result["structuredContent"]["projects"], json!([]));
        assert!(
            result["content"][0]["text"]
                .as_str()
                .unwrap()
                .contains("No matching visual editor comments are available.")
        );
    }

    #[test]
    fn handles_ping_notifications_and_unknown_methods() {
        let ping = handle_request(r#"{"jsonrpc":"2.0","id":"ping","method":"ping"}"#).unwrap();
        assert_eq!(ping["result"], json!({}));
        assert!(
            handle_request(r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#).is_none()
        );

        let unknown = handle_request(r#"{"jsonrpc":"2.0","id":3,"method":"unknown"}"#).unwrap();
        assert_eq!(unknown["error"]["code"], -32601);
    }

    #[test]
    fn returns_no_mentions_for_a_non_empty_path() {
        let result = call_tool(Some(&json!({
            "name": MENTION_SEARCH_TOOL,
            "arguments": { "path": ["nested"], "query": "" }
        })))
        .unwrap();

        assert_eq!(result["structuredContent"]["items"], json!([]));
    }

    #[test]
    fn formats_singular_and_plural_comment_counts() {
        for (count, expected) in [(0, "0 comments"), (1, "1 comment"), (2, "2 comments")] {
            assert_eq!(comment_count(count), expected);
        }
    }

    #[test]
    fn formats_comment_context_with_one_based_positions() {
        let project = ProjectComments {
            project_root: "/projects/slint".into(),
            comments: vec![EditorComment {
                id: "one".into(),
                text: "Align this with the toolbar.".into(),
                file: "/projects/slint/ui/main.slint".into(),
                range: slint_editor_mcp::SourceRange {
                    start: slint_editor_mcp::SourcePosition { line: 4, character: 2 },
                    end: slint_editor_mcp::SourcePosition { line: 8, character: 6 },
                },
                component: Some("MainWindow".into()),
                element_type: "Rectangle".into(),
                element_id: Some("toolbar".into()),
            }],
        };

        let markdown = format_project_comments(&project);
        for expected in [
            "Project: `/projects/slint`",
            "## Rectangle #toolbar in `MainWindow`",
            "- File: `/projects/slint/ui/main.slint`",
            "- Range: 5:3–9:7",
            "Align this with the toolbar.",
        ] {
            assert!(markdown.contains(expected), "missing {expected:?} in {markdown:?}");
        }
    }

    #[test]
    fn aggregates_editor_comments_into_parent_repository_directories() {
        let repository = tempfile::tempdir().unwrap();
        std::fs::create_dir(repository.path().join(".git")).unwrap();
        let gallery = repository.path().join("examples/gallery");
        let todo = repository.path().join("demos/todo");
        let examples = repository.path().join("examples");
        let demos = repository.path().join("demos");
        std::fs::create_dir_all(&gallery).unwrap();
        std::fs::create_dir_all(&todo).unwrap();

        let scopes = expand_comment_scopes(vec![
            ProjectComments {
                project_root: gallery.clone(),
                comments: vec![comment("gallery", gallery.join("main.slint"))],
            },
            ProjectComments {
                project_root: todo.clone(),
                comments: vec![comment("todo", todo.join("main.slint"))],
            },
        ]);
        let comments_in_scope = |directory: &Path| {
            scopes
                .iter()
                .find(|scope| scope.project_root == directory)
                .map(|scope| scope.comments.len())
        };

        for (directory, expected_count) in [
            (gallery.as_path(), Some(1)),
            (examples.as_path(), Some(1)),
            (todo.as_path(), Some(1)),
            (demos.as_path(), Some(1)),
            (repository.path(), Some(2)),
            (repository.path().parent().unwrap(), None),
        ] {
            assert_eq!(
                comments_in_scope(directory),
                expected_count,
                "unexpected scope {directory:?}"
            );
        }
    }
}
