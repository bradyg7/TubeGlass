//! Comments and replies, including sort options, pinned and hearted comments.

use super::*;

/// Finds the continuation token that starts the comments.
pub(crate) fn find_comments_token(v: &Value) -> Option<String> {
    match v {
        Value::Object(m) => {
            if m.get("targetId").and_then(Value::as_str) == Some("comments-section") {
                return m
                    .get("contents")?
                    .get(0)?
                    .pointer("/continuationItemRenderer/continuationEndpoint/continuationCommand/token")?
                    .as_str()
                    .map(String::from);
            }
            m.values().find_map(find_comments_token)
        }
        Value::Array(a) => a.iter().find_map(find_comments_token),
        _ => None,
    }
}

/// Collects every comment payload in a response.
pub(crate) fn collect_payloads<'a>(v: &'a Value, out: &mut Vec<&'a Value>) {
    match v {
        Value::Object(m) => {
            if let Some(p) = m.get("commentEntityPayload") {
                out.push(p);
            } else {
                m.values().for_each(|c| collect_payloads(c, out));
            }
        }
        Value::Array(a) => a.iter().for_each(|c| collect_payloads(c, out)),
        _ => {}
    }
}

/// The last continuation token in a response (the next page).
pub(crate) fn last_continuation(v: &Value) -> Option<String> {
    // The next-page token sits in the last continuationItemRenderer of the items array.
    let mut found = None;
    /// Depth-first search for a continuation token.
    fn walk(v: &Value, found: &mut Option<String>) {
        match v {
            Value::Object(m) => {
                if let Some(r) = m.get("continuationItemRenderer") {
                    let t = r
                        .pointer("/continuationEndpoint/continuationCommand/token")
                        .or_else(|| r.pointer("/button/buttonRenderer/command/continuationCommand/token"))
                        .and_then(Value::as_str);
                    if let Some(t) = t {
                        *found = Some(t.to_string());
                    }
                } else {
                    // do not descend into replies
                    for (k, c) in m {
                        if k != "replies" {
                            walk(c, found);
                        }
                    }
                }
            }
            Value::Array(a) => a.iter().for_each(|c| walk(c, found)),
            _ => {}
        }
    }
    walk(v, &mut found);
    found
}

/// Builds a Comment from its payload.
pub(crate) fn payload_to_comment(p: &Value, reply_token: Option<String>, pinned: Option<String>, hearted: bool) -> Comment {
    let st = |ptr: &str| p.pointer(ptr).and_then(Value::as_str).unwrap_or("").to_string();
    Comment {
        pinned,
        hearted: if hearted { Some(st("/toolbar/heartActiveTooltip")).filter(|t| !t.is_empty()) } else { None },
        by_creator: p.pointer("/author/isCreator").and_then(Value::as_bool).unwrap_or(false),
        verified: p.pointer("/author/isVerified").and_then(Value::as_bool).unwrap_or(false),
        id: st("/properties/commentId"),
        reply_token,
        author: st("/author/displayName"),
        avatar: st("/author/avatarThumbnailUrl"),
        text: st("/properties/content/content"),
        time: st("/properties/publishedTime"),
        likes: st("/toolbar/likeCountNotliked"),
        replies: st("/toolbar/replyCount"),
    }
}

/// (comment id, reply token, pinned text) for every thread in a response, in page order.
pub(crate) fn collect_threads(v: &Value, out: &mut Vec<(String, Option<String>, Option<String>)>) {
    match v {
        Value::Object(m) => {
            if let Some(t) = m.get("commentThreadRenderer") {
                let id = t
                    .pointer("/commentViewModel/commentViewModel/commentId")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string();
                let token = t
                    .pointer("/replies/commentRepliesRenderer/contents/0/continuationItemRenderer/continuationEndpoint/continuationCommand/token")
                    .and_then(Value::as_str)
                    .map(String::from);
                let pinned = t
                    .pointer("/commentViewModel/commentViewModel/pinnedText")
                    .and_then(Value::as_str)
                    .map(String::from);
                out.push((id, token, pinned));
            } else {
                m.values().for_each(|c| collect_threads(c, out));
            }
        }
        Value::Array(a) => a.iter().for_each(|c| collect_threads(c, out)),
        _ => {}
    }
}

/// toolbarStateKey -> hearted by the creator
pub(crate) fn collect_hearts(v: &Value, out: &mut std::collections::HashMap<String, bool>) {
    match v {
        Value::Object(m) => {
            if let Some(p) = m.get("engagementToolbarStateEntityPayload") {
                if let Some(k) = p["key"].as_str() {
                    out.insert(k.to_string(), p["heartState"].as_str() == Some("TOOLBAR_HEART_STATE_HEARTED"));
                }
            } else {
                m.values().for_each(|c| collect_hearts(c, out));
            }
        }
        Value::Array(a) => a.iter().for_each(|c| collect_hearts(c, out)),
        _ => {}
    }
}

/// Reads the Top / Newest sort choices.
pub(crate) fn find_sort_options(v: &Value) -> Vec<SortOpt> {
    match v {
        Value::Object(m) => {
            if let Some(items) = m.get("subMenuItems").and_then(Value::as_array) {
                let opts: Vec<SortOpt> = items
                    .iter()
                    .filter_map(|i| {
                        Some(SortOpt {
                            title: i["title"].as_str()?.to_string(),
                            token: i
                                .pointer("/serviceEndpoint/continuationCommand/token")
                                .and_then(Value::as_str)?
                                .to_string(),
                            selected: i["selected"].as_bool().unwrap_or(false),
                        })
                    })
                    .collect();
                if !opts.is_empty() {
                    return opts;
                }
            }
            m.values().map(find_sort_options).find(|o| !o.is_empty()).unwrap_or_default()
        }
        Value::Array(a) => a.iter().map(find_sort_options).find(|o| !o.is_empty()).unwrap_or_default(),
        _ => vec![],
    }
}

/// Top-level comments of a video. Pass the returned token back in to load more, or one of
/// the `sorts` tokens to switch the order.
pub fn comments_page(video_id: &str, token: Option<&str>) -> Res<CommentsPage> {
    let token = match token {
        Some(t) => t.to_string(),
        None => {
            let next = web_post("next", json!({ "videoId": video_id }))?;
            find_comments_token(&next).ok_or("comments are turned off for this video")?
        }
    };
    let resp = web_post("next", json!({ "continuation": token }))?;
    let mut payloads = vec![];
    collect_payloads(&resp, &mut payloads);
    let by_id: std::collections::HashMap<&str, &Value> = payloads
        .iter()
        .filter_map(|p| Some((p.pointer("/properties/commentId")?.as_str()?, *p)))
        .collect();
    let mut hearts = std::collections::HashMap::new();
    collect_hearts(&resp, &mut hearts);
    let hearted = |p: &Value| {
        p.pointer("/properties/toolbarStateKey")
            .and_then(Value::as_str)
            .and_then(|k| hearts.get(k))
            .copied()
            .unwrap_or(false)
    };
    let mut threads = vec![];
    collect_threads(&resp, &mut threads);
    let list: Vec<Comment> = if threads.is_empty() {
        payloads
            .iter()
            .filter(|p| p.pointer("/properties/replyLevel").and_then(Value::as_i64).unwrap_or(0) == 0)
            .map(|p| payload_to_comment(p, None, None, hearted(p)))
            .collect()
    } else {
        threads
            .into_iter()
            .filter_map(|(id, tok, pinned)| by_id.get(id.as_str()).map(|p| payload_to_comment(p, tok, pinned, hearted(p))))
            .collect()
    };
    Ok(CommentsPage { list, next: last_continuation(&resp), sorts: find_sort_options(&resp) })
}

/// Replies of one comment (`token` comes from `Comment::reply_token` or a previous page).
pub fn replies(token: &str) -> Res<(Vec<Comment>, Option<String>)> {
    let resp = web_post("next", json!({ "continuation": token }))?;
    let mut payloads = vec![];
    collect_payloads(&resp, &mut payloads);
    let mut hearts = std::collections::HashMap::new();
    collect_hearts(&resp, &mut hearts);
    let list = payloads
        .iter()
        .map(|p| {
            let hearted = p
                .pointer("/properties/toolbarStateKey")
                .and_then(Value::as_str)
                .and_then(|k| hearts.get(k))
                .copied()
                .unwrap_or(false);
            payload_to_comment(p, None, None, hearted)
        })
        .collect();
    Ok((list, last_continuation(&resp)))
}
