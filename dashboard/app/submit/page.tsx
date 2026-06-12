"use client";

import Link from "next/link";
import { useState } from "react";
import { REPO } from "@/lib/api";

const CATEGORIES = ["bug_fix", "feature", "refactor"];
const LANGUAGES = ["python", "go", "typescript", "javascript"];

export default function Submit() {
  const [id, setId] = useState("");
  const [category, setCategory] = useState("bug_fix");
  const [language, setLanguage] = useState("python");
  const [issue, setIssue] = useState("");

  function buildIssueUrl(): string {
    const title = `Task submission: ${id || "<task-id>"}`;
    const body = [
      `**Proposed task id:** \`${id || "<task-id>"}\``,
      `**Category:** ${category}`,
      `**Language:** ${language}`,
      "",
      "**Issue (what the agent is asked to do):**",
      "",
      issue || "<describe the problem; no solution>",
      "",
      "---",
      "Checklist (see `docs/TASK_CONTRIBUTION.md`):",
      "- [ ] `repo/` starting state + hidden `tests/` + `gold_patch.diff`",
      "- [ ] declarative `fail_to_pass` / `pass_to_pass` in `metadata.json`",
      "- [ ] honest `source` + `decontamination_notes`",
      "- [ ] `python scripts/validate_tasks.py tasks/v1/" + (id || "<task-id>") + "` passes",
    ].join("\n");
    const params = new URLSearchParams({ title, body, labels: "task-submission" });
    return `https://github.com/${REPO}/issues/new?${params.toString()}`;
  }

  const metaPreview = JSON.stringify(
    {
      id: id || "<task-id>",
      category,
      languages: [language],
      difficulty: "medium",
      created: "<YYYY-MM-DD>",
      source: "hand-authored",
      decontamination_notes: "<why this is original / post-cutoff>",
      tests: { fail_to_pass: [{ name: "...", cmd: "..." }], pass_to_pass: [] },
    },
    null,
    2,
  );

  return (
    <div className="min-h-screen bg-zinc-950 text-white p-8 font-sans">
      <div className="max-w-3xl mx-auto">
        <div className="flex justify-between items-baseline mb-8">
          <h1 className="text-4xl font-semibold tracking-tight">Submit a task</h1>
          <Link href="/tasks" className="text-sm text-zinc-400 hover:text-white">← tasks</Link>
        </div>

        <p className="text-sm text-zinc-400 mb-6">
          Tasks are how VulcanBench grows. Fill this in to open a pre-filled GitHub
          issue, then follow{" "}
          <a
            className="underline"
            href={`https://github.com/${REPO}/blob/main/docs/TASK_CONTRIBUTION.md`}
            target="_blank"
            rel="noopener noreferrer"
          >
            docs/TASK_CONTRIBUTION.md
          </a>{" "}
          to add the files and run the validator.
        </p>

        <div className="space-y-4">
          <label className="block">
            <span className="text-sm text-zinc-400">Task id</span>
            <input
              value={id}
              onChange={(e) => setId(e.target.value)}
              placeholder="py-some-fix"
              className="mt-1 w-full rounded-lg bg-black/40 border border-white/10 px-3 py-2 font-mono text-sm"
            />
          </label>

          <div className="grid grid-cols-2 gap-4">
            <label className="block">
              <span className="text-sm text-zinc-400">Category</span>
              <select
                value={category}
                onChange={(e) => setCategory(e.target.value)}
                className="mt-1 w-full rounded-lg bg-black/40 border border-white/10 px-3 py-2 text-sm"
              >
                {CATEGORIES.map((c) => <option key={c} value={c}>{c}</option>)}
              </select>
            </label>
            <label className="block">
              <span className="text-sm text-zinc-400">Language</span>
              <select
                value={language}
                onChange={(e) => setLanguage(e.target.value)}
                className="mt-1 w-full rounded-lg bg-black/40 border border-white/10 px-3 py-2 text-sm"
              >
                {LANGUAGES.map((l) => <option key={l} value={l}>{l}</option>)}
              </select>
            </label>
          </div>

          <label className="block">
            <span className="text-sm text-zinc-400">Issue (what the agent must do — no solution)</span>
            <textarea
              value={issue}
              onChange={(e) => setIssue(e.target.value)}
              rows={5}
              placeholder="Describe the bug or feature..."
              className="mt-1 w-full rounded-lg bg-black/40 border border-white/10 px-3 py-2 text-sm"
            />
          </label>

          <a
            href={buildIssueUrl()}
            target="_blank"
            rel="noreferrer"
            className="inline-flex h-11 items-center rounded-full bg-white px-6 text-sm font-medium text-black hover:bg-zinc-200"
          >
            Open pre-filled GitHub issue →
          </a>
        </div>

        <div className="mt-10">
          <div className="text-sm text-zinc-400 mb-2">metadata.json starting point</div>
          <pre className="bg-black/50 p-4 rounded-lg text-xs text-emerald-300 overflow-x-auto">{metaPreview}</pre>
        </div>
      </div>
    </div>
  );
}
