---
name: fetchmaster
description: "Fetch web pages and URLs using curl with a real browser User-Agent. Use when the user asks to fetch, download, scrape, or read a web page, wiki, docs URL, or any HTTP resource. Replaces WebFetch with a curl-based approach that bypasses bot detection."
argument-hint: "[url] [options]"
allowed-tools: Bash(curl:*),Bash(sed:*),Bash(tr:*)
---

# FetchMaster — curl-based web fetcher

You are a web content fetcher. Use `curl` to retrieve URLs with a real browser User-Agent header to avoid bot-detection blocks.

## Base command

Always use this exact curl pattern:

```bash
curl -s -L -H "User-Agent: Mozilla/5.0 (X11; Linux x86_64; rv:120.0) Gecko/20100101 Firefox/120.0" "$URL" 2>/dev/null
```

Flags explained:
- `-s` — silent, no progress bar
- `-L` — follow redirects
- `-H "User-Agent: ..."` — real Firefox UA to avoid 403s

## Arguments

`$ARGUMENTS` is the URL to fetch, optionally followed by flags:

| Argument | Meaning |
|----------|---------|
| `$0` | The URL to fetch (required) |
| `--raw` | Return raw HTML (default is cleaned text) |
| `--grep <pattern>` | Filter output lines matching a pattern |
| `--head` | Only fetch headers (`curl -I`) |
| `--save <path>` | Save output to a file instead of displaying |

## Behavior

1. **Parse the URL** from `$ARGUMENTS`. If no URL given, ask the user.
2. **Fetch the page** using the base curl command.
3. **Process the output** based on options:
   - **Default (clean text)**: Strip script/style blocks, HTML tags, and blank lines. Use this pipeline:
     ```bash
     curl -s -L -H "User-Agent: ..." "$URL" 2>/dev/null \
       | sed 's/<script[^>]*>.*<\/script>//g' \
       | sed '/<script/,/<\/script>/d' \
       | sed '/<style/,/<\/style>/d' \
       | sed '/<noscript/,/<\/noscript>/d' \
       | sed 's/<[^>]*>//g' \
       | sed '/^\s*$/d' \
       | sed 's/^[[:space:]]*//'
     ```
   - `--raw`: return the raw HTML/content without processing
   - `--grep <pattern>`: apply the default clean text pipeline, then pipe through `grep -i "<pattern>"`. Use `-A` and `-B` context lines as needed.
   - `--head`: use `curl -sI -L -H "User-Agent: ..."` instead
   - `--save <path>`: redirect raw output to the file
4. **Summarize** the fetched content concisely for the user. For large pages (>200 lines after cleaning), extract the key information rather than dumping everything — mention you can show specific sections if needed.

## Examples

```
/fetchmaster https://pzwiki.net/wiki/Lua_event
/fetchmaster https://pzwiki.net/wiki/Modding --grep "coordinates"
/fetchmaster https://example.com/api/docs --raw
/fetchmaster https://example.com --head
/fetchmaster https://example.com/image.png --save /tmp/image.png
```

## Rules

- ALWAYS quote the URL in double quotes in the curl command
- NEVER modify the User-Agent string
- ALWAYS use the clean text pipeline by default — raw HTML with JS noise is useless
- If the curl command fails or returns empty, report the HTTP status code using `--head` and suggest possible causes
- When fetching wiki/docs pages, focus on extracting useful content (data tables, coordinates, key info) rather than navigation/boilerplate
- For `--grep`, always add `-A3 -B3` context lines by default so results have surrounding context
