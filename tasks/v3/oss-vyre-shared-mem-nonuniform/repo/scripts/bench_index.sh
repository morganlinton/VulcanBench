#!/usr/bin/env bash
# Aggregates local Criterion reports into target/criterion/index.html.

set -euo pipefail

OUTPUT_DIR="target/criterion"
OUTPUT_FILE="$OUTPUT_DIR/index.html"
mkdir -p "$OUTPUT_DIR"

declare -A CODEBASES
CODEBASES["sofar"]="/home/mukund-thiru/sofar/target/criterion"
CODEBASES["spill"]="/home/mukund-thiru/spill/target/criterion"
CODEBASES["star-randsrv"]="/home/mukund-thiru/star-randsrv/target/criterion"
CODEBASES["vyre"]="/media/mukund-thiru/SanthData/Santh/libs/performance/matching/vyre/target/criterion"

write_line() {
    printf '%s\n' "$1" > "$OUTPUT_FILE"
}

append_line() {
    printf '%s\n' "$1" >> "$OUTPUT_FILE"
}

html_text() {
    local value="$1"
    value="${value//&/&amp;}"
    value="${value//</&lt;}"
    value="${value//>/&gt;}"
    value="${value//\"/&quot;}"
    printf '%s' "$value"
}

write_head() {
    write_line '<!DOCTYPE html>'
    append_line '<html lang="en">'
    append_line '<head>'
    append_line '    <meta charset="UTF-8">'
    append_line '    <meta name="viewport" content="width=device-width, initial-scale=1.0">'
    append_line '    <title>Aggregated Benchmark Reports</title>'
    append_line '    <link rel="preconnect" href="https://fonts.googleapis.com">'
    append_line '    <link rel="preconnect" href="https://fonts.gstatic.com" crossorigin>'
    append_line '    <link href="https://fonts.googleapis.com/css2?family=Outfit:wght@300;400;600;700&display=swap" rel="stylesheet">'
    append_line '    <style>'
    append_line '        :root {'
    append_line '            --bg-color: #0b0f19;'
    append_line '            --card-bg: rgba(255, 255, 255, 0.03);'
    append_line '            --card-border: rgba(255, 255, 255, 0.06);'
    append_line '            --primary-glow: linear-gradient(135deg, #6366f1 0%, #a855f7 100%);'
    append_line '            --text-main: #f3f4f6;'
    append_line '            --text-muted: #9ca3af;'
    append_line '            --accent-green: #10b981;'
    append_line '        }'
    append_line '        * {'
    append_line '            box-sizing: border-box;'
    append_line '            margin: 0;'
    append_line '            padding: 0;'
    append_line '        }'
    append_line '        body {'
    append_line "            font-family: 'Outfit', sans-serif;"
    append_line '            background-color: var(--bg-color);'
    append_line '            color: var(--text-main);'
    append_line '            line-height: 1.6;'
    append_line '            padding: 2rem 1rem;'
    append_line '            min-height: 100vh;'
    append_line '        }'
    append_line '        .container {'
    append_line '            max-width: 1200px;'
    append_line '            margin: 0 auto;'
    append_line '        }'
    append_line '        header {'
    append_line '            text-align: center;'
    append_line '            margin-bottom: 3.5rem;'
    append_line '            position: relative;'
    append_line '        }'
    append_line '        h1 {'
    append_line '            font-size: 2.75rem;'
    append_line '            font-weight: 700;'
    append_line '            background: var(--primary-glow);'
    append_line '            -webkit-background-clip: text;'
    append_line '            -webkit-text-fill-color: transparent;'
    append_line '            margin-bottom: 0.5rem;'
    append_line '            letter-spacing: -0.025em;'
    append_line '        }'
    append_line '        .subtitle {'
    append_line '            color: var(--text-muted);'
    append_line '            font-size: 1.1rem;'
    append_line '            font-weight: 300;'
    append_line '        }'
    append_line '        .grid {'
    append_line '            display: grid;'
    append_line '            grid-template-columns: repeat(auto-fit, minmax(320px, 1fr));'
    append_line '            gap: 1.75rem;'
    append_line '        }'
    append_line '        .codebase-card {'
    append_line '            background: var(--card-bg);'
    append_line '            border: 1px solid var(--card-border);'
    append_line '            border-radius: 16px;'
    append_line '            padding: 1.75rem;'
    append_line '            backdrop-filter: blur(12px);'
    append_line '            transition: transform 0.3s cubic-bezier(0.4, 0, 0.2, 1), border-color 0.3s ease, box-shadow 0.3s ease;'
    append_line '            position: relative;'
    append_line '            overflow: hidden;'
    append_line '        }'
    append_line '        .codebase-card::before {'
    append_line "            content: '';"
    append_line '            position: absolute;'
    append_line '            top: 0;'
    append_line '            left: 0;'
    append_line '            width: 100%;'
    append_line '            height: 4px;'
    append_line '            background: var(--primary-glow);'
    append_line '            opacity: 0;'
    append_line '            transition: opacity 0.3s ease;'
    append_line '        }'
    append_line '        .codebase-card:hover {'
    append_line '            transform: translateY(-4px);'
    append_line '            border-color: rgba(99, 102, 241, 0.4);'
    append_line '            box-shadow: 0 12px 30px rgba(0, 0, 0, 0.5), 0 0 1px rgba(99, 102, 241, 0.4);'
    append_line '        }'
    append_line '        .codebase-card:hover::before {'
    append_line '            opacity: 1;'
    append_line '        }'
    append_line '        .codebase-title {'
    append_line '            font-size: 1.5rem;'
    append_line '            font-weight: 600;'
    append_line '            margin-bottom: 1rem;'
    append_line '            display: flex;'
    append_line '            align-items: center;'
    append_line '            justify-content: space-between;'
    append_line '        }'
    append_line '        .badge {'
    append_line '            font-size: 0.75rem;'
    append_line '            font-weight: 600;'
    append_line '            background: rgba(99, 102, 241, 0.15);'
    append_line '            color: #818cf8;'
    append_line '            padding: 0.25rem 0.75rem;'
    append_line '            border-radius: 9999px;'
    append_line '            border: 1px solid rgba(99, 102, 241, 0.3);'
    append_line '            text-transform: uppercase;'
    append_line '            letter-spacing: 0.05em;'
    append_line '        }'
    append_line '        .bench-list {'
    append_line '            list-style: none;'
    append_line '            display: flex;'
    append_line '            flex-direction: column;'
    append_line '            gap: 0.75rem;'
    append_line '        }'
    append_line '        .bench-item {'
    append_line '            background: rgba(255, 255, 255, 0.02);'
    append_line '            border: 1px solid rgba(255, 255, 255, 0.03);'
    append_line '            border-radius: 8px;'
    append_line '            padding: 0.75rem 1rem;'
    append_line '            display: flex;'
    append_line '            align-items: center;'
    append_line '            justify-content: space-between;'
    append_line '            transition: background 0.2s ease, border-color 0.2s ease;'
    append_line '        }'
    append_line '        .bench-item:hover {'
    append_line '            background: rgba(255, 255, 255, 0.05);'
    append_line '            border-color: rgba(255, 255, 255, 0.08);'
    append_line '        }'
    append_line '        .bench-link {'
    append_line '            color: var(--text-main);'
    append_line '            text-decoration: none;'
    append_line '            font-size: 0.95rem;'
    append_line '            font-weight: 400;'
    append_line '            display: flex;'
    append_line '            align-items: center;'
    append_line '            gap: 0.5rem;'
    append_line '            width: 100%;'
    append_line '        }'
    append_line '        .bench-link::after {'
    append_line "            content: '->';"
    append_line '            margin-left: auto;'
    append_line '            color: var(--text-muted);'
    append_line '            transition: transform 0.2s ease, color 0.2s ease;'
    append_line '        }'
    append_line '        .bench-link:hover::after {'
    append_line '            transform: translateX(3px);'
    append_line '            color: #818cf8;'
    append_line '        }'
    append_line '        .no-benches {'
    append_line '            color: var(--text-muted);'
    append_line '            font-size: 0.9rem;'
    append_line '            font-style: italic;'
    append_line '        }'
    append_line '        footer {'
    append_line '            margin-top: 5rem;'
    append_line '            text-align: center;'
    append_line '            color: var(--text-muted);'
    append_line '            font-size: 0.85rem;'
    append_line '            font-weight: 300;'
    append_line '            border-top: 1px solid rgba(255, 255, 255, 0.05);'
    append_line '            padding-top: 1.5rem;'
    append_line '        }'
    append_line '    </style>'
    append_line '</head>'
    append_line '<body>'
    append_line '    <div class="container">'
    append_line '        <header>'
    append_line '            <h1>Criterion Benchmark Reports</h1>'
    append_line '            <p class="subtitle">Aggregated performance insights across workspaces</p>'
    append_line '        </header>'
    append_line '        <div class="grid">'
}

append_card_start() {
    local name="$1"
    local escaped_name
    escaped_name="$(html_text "$name")"
    append_line '            <div class="codebase-card">'
    append_line '                <div class="codebase-title">'
    append_line "                    <span>$escaped_name</span>"
    append_line '                    <span class="badge">Crate</span>'
    append_line '                </div>'
    append_line '                <ul class="bench-list">'
}

append_bench_link() {
    local report_path="$1"
    local bench_name="$2"
    local escaped_name
    escaped_name="$(html_text "$bench_name")"
    append_line '                    <li class="bench-item">'
    append_line "                        <a class=\"bench-link\" href=\"file://$report_path\" target=\"_blank\">$escaped_name</a>"
    append_line '                    </li>'
}

append_empty_bench_message() {
    append_line '                    <li class="no-benches">No local benchmarks found. Run ./cargo_full bench to generate reports.</li>'
}

append_card_end() {
    append_line '                </ul>'
    append_line '            </div>'
}

write_tail() {
    append_line '        </div>'
    append_line '        <footer>'
    append_line '            <p>Aggregated automatically &bull; Design: Antigravity</p>'
    append_line '        </footer>'
    append_line '    </div>'
    append_line '</body>'
    append_line '</html>'
}

write_head

for name in "sofar" "spill" "star-randsrv" "vyre"; do
    path="${CODEBASES[$name]}"
    echo "Processing benchmarks for $name..."
    append_card_start "$name"

    benches_found=0
    if [[ -d "$path" ]]; then
        while IFS= read -r dir; do
            [[ -z "$dir" ]] && continue
            bench_name=$(basename "$dir")
            if [[ "$bench_name" == "report" || "$bench_name" == "custom" ]]; then
                continue
            fi

            if [[ -f "$dir/report/index.html" ]]; then
                report_path="$dir/report/index.html"
                benches_found=$((benches_found + 1))
                append_bench_link "$report_path" "$bench_name"
            fi
        done < <(find "$path" -maxdepth 1 -type d 2>/dev/null | sort)
    fi

    if [[ "$benches_found" -eq 0 ]]; then
        append_empty_bench_message
    fi

    append_card_end
done

write_tail

echo "Benchmark index page generated successfully at file://$(pwd)/$OUTPUT_FILE"
