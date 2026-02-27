<script lang="ts">
    import { onMount, onDestroy } from "svelte";
    import { fade, fly } from "svelte/transition";
    import { cubicOut } from "svelte/easing";
    import { page } from "$app/stores";
    import {
        loadProject,
        nodes,
        saveNode,
        readOnly,
        canEditNode,
        accessControl,
        activeAccessProfile,
    } from "$lib/store/model";
    import { invoke } from "@tauri-apps/api/core";
    import type {
        Node,
        KnowledgeBodyFormat,
        RequirementData,
        SubsystemArtifact,
        SubsystemActivity,
        SubsystemKnowledgePage,
    } from "$lib/types";

    // ── Artifact types ─────────────────────────────────────────────────────────

    type ArtifactType =
        | "doc"
        | "whitepaper"
        | "journal"
        | "website"
        | "jira"
        | "github"
        | "svn"
        | "test"
        | "model";

    type Artifact = SubsystemArtifact;
    type KnowledgePage = SubsystemKnowledgePage;
    type ActivityEntry = SubsystemActivity;
    type KbFormat = KnowledgeBodyFormat;
    type KbEditorView = "write" | "preview" | "split";

    const TYPE_OPTIONS: { value: ArtifactType; label: string }[] = [
        { value: "doc", label: "Document" },
        { value: "whitepaper", label: "White Paper" },
        { value: "journal", label: "Journal" },
        { value: "website", label: "Website" },
        { value: "jira", label: "Jira Ticket" },
        { value: "github", label: "GitHub Repo/Issue" },
        { value: "svn", label: "SVN Repo" },
        { value: "test", label: "Test / Simulation" },
        { value: "model", label: "Model / Diagram" },
    ];

    const TYPE_ICON: Record<string, string> = {
        doc: "📄",
        whitepaper: "📋",
        journal: "📓",
        website: "🌐",
        jira: "🎫",
        github: "🐙",
        svn: "🔗",
        test: "🧪",
        model: "🔷",
    };

    // ── Route params ───────────────────────────────────────────────────────────

    $: projectId = $page.params.id;
    $: subsystemId = $page.params.nodeId;

    // ── Subsystem node ─────────────────────────────────────────────────────────

    let subsystemName = "Subsystem";
    let subsystemDescription = "";
    let editingDesc = false;
    let descDraft = "";

    // ── Tabs ───────────────────────────────────────────────────────────────────

    type Tab =
        | "overview"
        | "board"
        | "requirements"
        | "knowledge"
        | "artifacts";
    let activeTab: Tab = "overview";
    type BoardLane = "todo" | "doing" | "done";
    type BoardPriority = "none" | "p0" | "p1" | "p2" | "p3";
    const BOARD_STATUS_OPTIONS = [
        { value: "todo", label: "To Do" },
        { value: "doing", label: "In Progress" },
        { value: "done", label: "Done" },
    ] as const;
    const BOARD_PRIORITY_OPTIONS: { value: BoardPriority; label: string }[] = [
        { value: "none", label: "No Priority" },
        { value: "p0", label: "P0 Critical" },
        { value: "p1", label: "P1 High" },
        { value: "p2", label: "P2 Medium" },
        { value: "p3", label: "P3 Low" },
    ];
    const BOARD_LANES: { value: BoardLane; label: string }[] = [
        { value: "todo", label: "To Do" },
        { value: "doing", label: "In Progress" },
        { value: "done", label: "Done" },
    ];

    // ── Requirements ──────────────────────────────────────────────────────────

    $: allRequirements = $nodes.filter(
        (n) => n.kind === "requirement",
    ) as Node[];

    // Re-derive subsystemName reactively from the store so allocation counts
    // update immediately when $nodes changes (e.g. after a flow-down).
    $: subsystemName =
        $nodes.find((n) => n.id === subsystemId)?.name ?? subsystemName;
    $: scopedSubsystemNode = $nodes.find((n) => n.id === subsystemId) ?? null;
    $: canEditScopedSubsystem = scopedSubsystemNode
        ? canEditNode(scopedSubsystemNode)
        : false;
    $: isSubsystemWorkspaceMode =
        $accessControl.enabled && $activeAccessProfile.role === "subsystem";

    function reqData(n: Node): RequirementData {
        return n.data as RequirementData;
    }

    function isAllocated(n: Node, key: string): boolean {
        const allocs = reqData(n).allocations ?? [];
        return allocs.some(
            (a) => a.trim().toLowerCase() === key.trim().toLowerCase(),
        );
    }

    $: allocatedReqs = allRequirements.filter((n) =>
        isAllocated(n, subsystemName),
    );
    // Only show requirements with NO allocation at all — ones already owned by
    // another subsystem are excluded so they can't be double-assigned.
    $: unallocatedReqs = allRequirements.filter((n) => {
        const allocs = reqData(n).allocations ?? [];
        return allocs.length === 0;
    });
    function boardLaneForNode(node: Node): BoardLane {
        const meta = (node.meta as Record<string, unknown> | undefined) ?? {};
        const lane = String(meta.subsystem_board_state ?? "")
            .trim()
            .toLowerCase();
        if (lane === "todo" || lane === "doing" || lane === "done") return lane;
        return reqData(node).status === "approved" ? "done" : "todo";
    }

    function boardReqsForLane(lane: BoardLane): Node[] {
        if (lane === "todo") return boardTodoReqs;
        if (lane === "doing") return boardDoingReqs;
        return boardDoneReqs;
    }

    function reqMeta(node: Node): Record<string, unknown> {
        return (node.meta as Record<string, unknown> | undefined) ?? {};
    }

    function boardPriorityForNode(node: Node): BoardPriority {
        const raw = String(reqMeta(node).subsystem_board_priority ?? "")
            .trim()
            .toLowerCase();
        if (raw === "p0" || raw === "p1" || raw === "p2" || raw === "p3")
            return raw;
        return "none";
    }

    function boardAssigneeForNode(node: Node): string {
        return String(reqMeta(node).subsystem_assignee ?? "").trim();
    }

    function boardDueDateForNode(node: Node): string {
        const raw = String(reqMeta(node).subsystem_due_date ?? "").trim();
        if (!raw) return "";
        if (/^\d{4}-\d{2}-\d{2}$/.test(raw)) return raw;
        const parsed = new Date(raw);
        if (Number.isNaN(parsed.getTime())) return "";
        return parsed.toISOString().slice(0, 10);
    }

    function boardEstimateForNode(node: Node): string {
        return String(reqMeta(node).subsystem_estimate ?? "").trim();
    }

    function parseBoardLabels(value: string): string[] {
        const labels = value
            .split(",")
            .map((l) => l.trim())
            .filter(Boolean);
        return Array.from(new Set(labels)).slice(0, 16);
    }

    function boardLabelsForNode(node: Node): string[] {
        const raw = reqMeta(node).subsystem_labels;
        if (Array.isArray(raw)) {
            return Array.from(
                new Set(raw.map((v) => String(v).trim()).filter(Boolean)),
            ).slice(0, 16);
        }
        if (typeof raw === "string") return parseBoardLabels(raw);
        return [];
    }

    function boardLabelsCsvForNode(node: Node): string {
        return boardLabelsForNode(node).join(", ");
    }

    async function setRequirementBoardMeta(
        req: Node,
        patch: Record<string, unknown>,
        activityText: string,
    ) {
        if ($readOnly || !canEditNode(req)) return;
        const priorMeta = reqMeta(req);
        const changed = Object.entries(patch).some(([key, value]) => {
            const prev = priorMeta[key];
            return JSON.stringify(prev) !== JSON.stringify(value);
        });
        if (!changed) return;

        const updated: Node = {
            ...req,
            modified_at: new Date().toISOString(),
            meta: { ...priorMeta, ...patch },
        };
        await saveNode(updated);
        addActivity(activityText);
    }
    $: boardTodoReqs = allocatedReqs.filter(
        (n) => boardLaneForNode(n) === "todo",
    );
    $: boardDoingReqs = allocatedReqs.filter(
        (n) => boardLaneForNode(n) === "doing",
    );
    $: boardDoneReqs = allocatedReqs.filter(
        (n) => boardLaneForNode(n) === "done",
    );

    async function setRequirementBoardLane(req: Node, nextLane: string) {
        if ($readOnly || !canEditNode(req)) return;
        if (nextLane !== "todo" && nextLane !== "doing" && nextLane !== "done")
            return;
        if (boardLaneForNode(req) === nextLane) return;
        const updated: Node = {
            ...req,
            modified_at: new Date().toISOString(),
            meta: { ...(req.meta ?? {}), subsystem_board_state: nextLane },
        };
        await saveNode(updated);
        addActivity(`Moved requirement "${req.name}" to ${nextLane}.`);
    }

    function onBoardLaneChange(req: Node, event: Event) {
        const target = event.currentTarget as HTMLSelectElement | null;
        if (!target) return;
        void setRequirementBoardLane(req, target.value);
    }

    function onBoardPriorityChange(req: Node, event: Event) {
        const target = event.currentTarget as HTMLSelectElement | null;
        if (!target) return;
        const value = target.value as BoardPriority;
        if (!BOARD_PRIORITY_OPTIONS.some((opt) => opt.value === value)) return;
        void setRequirementBoardMeta(
            req,
            { subsystem_board_priority: value },
            `Updated priority for requirement "${req.name}" to ${value.toUpperCase()}.`,
        );
    }

    function onBoardAssigneeChange(req: Node, event: Event) {
        const target = event.currentTarget as HTMLInputElement | null;
        if (!target) return;
        const value = target.value.trim();
        void setRequirementBoardMeta(
            req,
            { subsystem_assignee: value },
            value
                ? `Assigned requirement "${req.name}" to ${value}.`
                : `Cleared assignee for requirement "${req.name}".`,
        );
    }

    function onBoardDueDateChange(req: Node, event: Event) {
        const target = event.currentTarget as HTMLInputElement | null;
        if (!target) return;
        const value = target.value.trim();
        if (value && !/^\d{4}-\d{2}-\d{2}$/.test(value)) return;
        void setRequirementBoardMeta(
            req,
            { subsystem_due_date: value },
            value
                ? `Set due date for requirement "${req.name}" to ${value}.`
                : `Cleared due date for requirement "${req.name}".`,
        );
    }

    function onBoardEstimateChange(req: Node, event: Event) {
        const target = event.currentTarget as HTMLInputElement | null;
        if (!target) return;
        const value = target.value.trim();
        void setRequirementBoardMeta(
            req,
            { subsystem_estimate: value },
            value
                ? `Set estimate for requirement "${req.name}" to ${value}.`
                : `Cleared estimate for requirement "${req.name}".`,
        );
    }

    function onBoardLabelsChange(req: Node, event: Event) {
        const target = event.currentTarget as HTMLInputElement | null;
        if (!target) return;
        const labels = parseBoardLabels(target.value);
        void setRequirementBoardMeta(
            req,
            { subsystem_labels: labels },
            labels.length > 0
                ? `Updated labels for requirement "${req.name}".`
                : `Cleared labels for requirement "${req.name}".`,
        );
    }

    async function flowDown(req: Node) {
        if ($readOnly || !canEditNode(req)) return;
        const data = reqData(req);
        const updated: Node = {
            ...req,
            modified_at: new Date().toISOString(),
            data: { ...data, allocations: [subsystemName.trim()] },
        };
        await saveNode(updated);
        addActivity(
            `Flowed down requirement "${req.name}" (${data.req_id ?? "REQ-?"}) to ${subsystemName}.`,
        );
    }

    async function removeAllocation(req: Node) {
        if ($readOnly || !canEditNode(req)) return;
        const data = reqData(req);
        const key = subsystemName.trim().toLowerCase();
        const allocs = (data.allocations ?? []).filter(
            (a) => a.trim().toLowerCase() !== key,
        );
        const updated: Node = {
            ...req,
            modified_at: new Date().toISOString(),
            data: { ...data, allocations: allocs },
        };
        await saveNode(updated);
        addActivity(
            `Removed requirement "${req.name}" (${data.req_id ?? "REQ-?"}) from ${subsystemName}.`,
        );
    }

    // ── Knowledge pages ────────────────────────────────────────────────────────

    const KB_FORMAT_OPTIONS: { value: KbFormat; label: string }[] = [
        { value: "plain", label: "Plain Text" },
        { value: "markdown", label: "Markdown" },
        { value: "rich", label: "Rich Text" },
    ];

    let knowledgePages: KnowledgePage[] = [];
    let activePageId: string | null = null;
    let pageEditing = false;
    let pageTitleDraft = "";
    let pageBodyDraft = "";
    let pageFormatDraft: KbFormat = "plain";
    let kbEditorView: KbEditorView = "split";
    let showNewPageModal = false;
    let newPageTitle = "";
    let knowledgeImportInput: HTMLInputElement | null = null;
    let knowledgeImportError: string | null = null;
    let richEditorEl: HTMLDivElement | null = null;

    $: activePage = knowledgePages.find((p) => p.id === activePageId) ?? null;

    async function refreshKnowledge(id: string) {
        knowledgePages = await invoke<KnowledgePage[]>(
            "list_subsystem_knowledge",
            {
                subsystemId: id,
            },
        );
    }

    async function createPage() {
        if ($readOnly || !canEditScopedSubsystem) return;
        if (!newPageTitle.trim()) return;
        const np: KnowledgePage = {
            id: crypto.randomUUID(),
            subsystem_id: subsystemId,
            title: newPageTitle.trim(),
            body: "",
            body_format: "plain",
            created_at: new Date().toISOString(),
            updated_at: new Date().toISOString(),
        };
        knowledgePages = [np, ...knowledgePages];
        activePageId = np.id;
        pageEditing = true;
        pageTitleDraft = np.title;
        pageBodyDraft = np.body;
        pageFormatDraft = normalizeKnowledgeFormat(np.body_format);
        newPageTitle = "";
        showNewPageModal = false;
        addActivity(`Created knowledge page "${np.title}".`);
        await invoke("upsert_subsystem_knowledge", { page: np });
    }

    function openPage(p: KnowledgePage) {
        activePageId = p.id;
        pageEditing = false;
        pageTitleDraft = p.title;
        pageBodyDraft = p.body;
        pageFormatDraft = normalizeKnowledgeFormat(p.body_format);
        knowledgeImportError = null;
    }

    function startEditPage() {
        if (!activePage || !canEditScopedSubsystem) return;
        pageTitleDraft = activePage.title;
        pageBodyDraft = activePage.body;
        pageFormatDraft = normalizeKnowledgeFormat(activePage.body_format);
        pageEditing = true;
        kbEditorView = pageFormatDraft === "plain" ? "write" : "split";
        knowledgeImportError = null;
    }

    async function savePage() {
        if ($readOnly || !canEditScopedSubsystem) return;
        if (!activePageId) return;
        const updated = knowledgePages.map((p) =>
            p.id === activePageId
                ? {
                      ...p,
                      title: pageTitleDraft.trim() || p.title,
                      body: pageBodyDraft,
                      body_format: pageFormatDraft,
                      updated_at: new Date().toISOString(),
                  }
                : p,
        );
        knowledgePages = updated;
        const page = updated.find((p) => p.id === activePageId);
        if (page) await invoke("upsert_subsystem_knowledge", { page });
        pageEditing = false;
        addActivity(`Updated knowledge page "${pageTitleDraft.trim()}".`);
    }

    function normalizeKnowledgeFormat(
        value: string | undefined | null,
    ): KbFormat {
        if (value === "markdown" || value === "rich") return value;
        return "plain";
    }

    function escapeHtml(text: string): string {
        return text
            .replace(/&/g, "&amp;")
            .replace(/</g, "&lt;")
            .replace(/>/g, "&gt;")
            .replace(/"/g, "&quot;")
            .replace(/'/g, "&#39;");
    }

    function htmlToPlainText(html: string): string {
        if (typeof window === "undefined") return html;
        const doc = new DOMParser().parseFromString(html, "text/html");
        return (doc.body.textContent ?? "").replace(/\u00a0/g, " ");
    }

    function plainTextToRichHtml(text: string): string {
        const blocks = text
            .replace(/\r\n/g, "\n")
            .split(/\n{2,}/)
            .map((b) => b.trim())
            .filter(Boolean);
        if (blocks.length === 0) return "";
        return blocks
            .map((b) => `<p>${escapeHtml(b).replace(/\n/g, "<br>")}</p>`)
            .join("");
    }

    function convertBodyForFormat(
        body: string,
        fromFormat: KbFormat,
        toFormat: KbFormat,
    ): string {
        if (fromFormat === toFormat) return body;
        if (toFormat === "rich") {
            if (fromFormat === "rich") return body;
            return plainTextToRichHtml(body);
        }
        if (fromFormat === "rich") {
            return htmlToPlainText(body);
        }
        return body;
    }

    function onKnowledgeFormatChange(next: string) {
        if (next !== "plain" && next !== "markdown" && next !== "rich") return;
        const toFormat = next as KbFormat;
        const fromFormat = pageFormatDraft;
        pageBodyDraft = convertBodyForFormat(
            pageBodyDraft,
            fromFormat,
            toFormat,
        );
        pageFormatDraft = toFormat;
        if (toFormat === "plain" && kbEditorView !== "write")
            kbEditorView = "write";
        if (toFormat !== "plain" && kbEditorView === "write")
            kbEditorView = "split";
    }

    function onKnowledgeFormatSelectChange(event: Event) {
        const target = event.currentTarget as HTMLSelectElement | null;
        if (!target) return;
        onKnowledgeFormatChange(target.value);
    }

    function sanitizeHtml(raw: string): string {
        if (typeof window === "undefined") return escapeHtml(raw);

        const allowedTags = new Set([
            "P",
            "BR",
            "DIV",
            "SPAN",
            "STRONG",
            "B",
            "EM",
            "I",
            "U",
            "S",
            "UL",
            "OL",
            "LI",
            "A",
            "H1",
            "H2",
            "H3",
            "H4",
            "BLOCKQUOTE",
            "CODE",
            "PRE",
            "HR",
        ]);
        const allowedAttrs = new Map<string, Set<string>>([
            ["A", new Set(["href", "title", "target", "rel"])],
            ["SPAN", new Set(["class"])],
            ["DIV", new Set(["class"])],
            ["CODE", new Set(["class"])],
            ["PRE", new Set(["class"])],
            ["P", new Set(["class"])],
        ]);

        const doc = new DOMParser().parseFromString(
            `<div>${raw}</div>`,
            "text/html",
        );
        const root = doc.body.firstElementChild as HTMLElement | null;
        if (!root) return "";

        const allElements = Array.from(root.querySelectorAll("*"));
        for (const el of allElements) {
            const tag = el.tagName.toUpperCase();
            if (!allowedTags.has(tag)) {
                const parent = el.parentNode;
                if (!parent) continue;
                while (el.firstChild) parent.insertBefore(el.firstChild, el);
                parent.removeChild(el);
                continue;
            }

            const allowed = allowedAttrs.get(tag) ?? new Set<string>();
            for (const attr of Array.from(el.attributes)) {
                const name = attr.name.toLowerCase();
                const isEventAttr = name.startsWith("on");
                if (isEventAttr || !allowed.has(attr.name)) {
                    el.removeAttribute(attr.name);
                }
            }

            if (tag === "A") {
                const href = el.getAttribute("href") ?? "";
                const lowerHref = href.trim().toLowerCase();
                if (
                    lowerHref.startsWith("javascript:") ||
                    lowerHref.startsWith("data:") ||
                    lowerHref.startsWith("vbscript:")
                ) {
                    el.removeAttribute("href");
                } else if (href) {
                    el.setAttribute("target", "_blank");
                    el.setAttribute("rel", "noopener noreferrer");
                }
            }
        }

        return root.innerHTML;
    }

    function extractLatexTokens(source: string): {
        text: string;
        inline: string[];
        block: string[];
    } {
        const inline: string[] = [];
        const block: string[] = [];
        let text = source;

        text = text.replace(/\$\$([\s\S]+?)\$\$/g, (_m, expr: string) => {
            const idx = block.push(expr.trim()) - 1;
            return `@@KB_LATEX_BLOCK_${idx}@@`;
        });

        text = text.replace(/\$([^$\n]+?)\$/g, (_m, expr: string) => {
            const idx = inline.push(expr.trim()) - 1;
            return `@@KB_LATEX_INLINE_${idx}@@`;
        });

        return { text, inline, block };
    }

    function restoreLatexTokens(
        html: string,
        inline: string[],
        block: string[],
    ): string {
        let out = html;
        out = out.replace(/@@KB_LATEX_BLOCK_(\d+)@@/g, (_m, idxRaw: string) => {
            const idx = Number(idxRaw);
            const expr = block[idx] ?? "";
            return `<pre class="kb-latex-block"><code>${escapeHtml(expr)}</code></pre>`;
        });
        out = out.replace(
            /@@KB_LATEX_INLINE_(\d+)@@/g,
            (_m, idxRaw: string) => {
                const idx = Number(idxRaw);
                const expr = inline[idx] ?? "";
                return `<span class="kb-latex-inline">${escapeHtml(expr)}</span>`;
            },
        );
        return out;
    }

    function renderInlineMarkdown(line: string): string {
        let out = escapeHtml(line);
        out = out.replace(/`([^`]+)`/g, "<code>$1</code>");
        out = out.replace(/\*\*([^*]+)\*\*/g, "<strong>$1</strong>");
        out = out.replace(/__([^_]+)__/g, "<strong>$1</strong>");
        out = out.replace(/\*([^*]+)\*/g, "<em>$1</em>");
        out = out.replace(/_([^_]+)_/g, "<em>$1</em>");
        out = out.replace(/~~([^~]+)~~/g, "<s>$1</s>");
        out = out.replace(/\[([^\]]+)\]\(([^)]+)\)/g, '<a href="$2">$1</a>');
        return out;
    }

    function renderMarkdownBody(markdown: string): string {
        const { text, inline, block } = extractLatexTokens(
            markdown.replace(/\r\n/g, "\n"),
        );
        const lines = text.split("\n");
        const html: string[] = [];
        let inCode = false;
        let codeLines: string[] = [];
        let inUl = false;
        let inOl = false;

        const closeLists = () => {
            if (inUl) html.push("</ul>");
            if (inOl) html.push("</ol>");
            inUl = false;
            inOl = false;
        };

        for (const lineRaw of lines) {
            const line = lineRaw ?? "";
            const trimmed = line.trim();

            if (trimmed.startsWith("```")) {
                closeLists();
                if (!inCode) {
                    inCode = true;
                    codeLines = [];
                } else {
                    html.push(
                        `<pre><code>${escapeHtml(codeLines.join("\n"))}</code></pre>`,
                    );
                    inCode = false;
                    codeLines = [];
                }
                continue;
            }
            if (inCode) {
                codeLines.push(line);
                continue;
            }

            const heading = trimmed.match(/^(#{1,4})\s+(.*)$/);
            if (heading) {
                closeLists();
                const level = heading[1].length;
                html.push(
                    `<h${level}>${renderInlineMarkdown(heading[2])}</h${level}>`,
                );
                continue;
            }

            const ul = trimmed.match(/^[-*+]\s+(.*)$/);
            if (ul) {
                if (inOl) {
                    html.push("</ol>");
                    inOl = false;
                }
                if (!inUl) {
                    html.push("<ul>");
                    inUl = true;
                }
                html.push(`<li>${renderInlineMarkdown(ul[1])}</li>`);
                continue;
            }

            const ol = trimmed.match(/^\d+\.\s+(.*)$/);
            if (ol) {
                if (inUl) {
                    html.push("</ul>");
                    inUl = false;
                }
                if (!inOl) {
                    html.push("<ol>");
                    inOl = true;
                }
                html.push(`<li>${renderInlineMarkdown(ol[1])}</li>`);
                continue;
            }

            const quote = trimmed.match(/^>\s?(.*)$/);
            if (quote) {
                closeLists();
                html.push(
                    `<blockquote>${renderInlineMarkdown(quote[1])}</blockquote>`,
                );
                continue;
            }

            if (!trimmed) {
                closeLists();
                html.push('<div class="kb-para-gap"></div>');
                continue;
            }

            closeLists();
            html.push(
                `<p class="kb-para">${renderInlineMarkdown(trimmed)}</p>`,
            );
        }

        closeLists();
        if (inCode) {
            html.push(
                `<pre><code>${escapeHtml(codeLines.join("\n"))}</code></pre>`,
            );
        }
        return restoreLatexTokens(html.join(""), inline, block);
    }

    function renderKnowledgeBody(body: string, format: KbFormat): string {
        if (!body.trim()) return "";
        if (format === "rich") {
            const tokens = extractLatexTokens(body);
            const withMath = restoreLatexTokens(
                tokens.text,
                tokens.inline,
                tokens.block,
            );
            return sanitizeHtml(withMath);
        }
        if (format === "markdown") {
            return sanitizeHtml(renderMarkdownBody(body));
        }
        const safe = escapeHtml(body).replace(/\r\n/g, "\n");
        return safe
            .split("\n")
            .map((line) =>
                line.trim()
                    ? `<p class="kb-para">${line}</p>`
                    : '<div class="kb-para-gap"></div>',
            )
            .join("");
    }

    $: activePageRenderedBody = activePage
        ? renderKnowledgeBody(
              activePage.body,
              normalizeKnowledgeFormat(activePage.body_format),
          )
        : "";
    $: draftRenderedBody = renderKnowledgeBody(pageBodyDraft, pageFormatDraft);

    function applyRichCommand(command: string, value?: string) {
        if (!richEditorEl) return;
        richEditorEl.focus();
        document.execCommand(command, false, value);
        pageBodyDraft = richEditorEl.innerHTML;
    }

    function syncRichEditorDraft() {
        if (!richEditorEl) return;
        pageBodyDraft = richEditorEl.innerHTML;
    }

    function fixEncoding(text: string): string {
        return text
            .replace(/Ã‚Â°/g, "\u00B0")
            .replace(/Ã¢â‚¬"/g, "-")
            .replace(/Ã¢â‚¬Ëœ/g, "'")
            .replace(/Ã¢â‚¬â„¢/g, "'")
            .replace(/Ã¢â‚¬Å“/g, '"')
            .replace(/Ã¢â‚¬/g, '"')
            .replace(/Ã‚Âµ/g, "\u00B5")
            .replace(/Ã‚Â·/g, "\u00B7")
            .replace(/Ã‚Â±/g, "\u00B1")
            .replace(/Ãƒâ€”/g, "x")
            .replace(/ÃƒÂ©/g, "e")
            .replace(/\u00ad/g, "")
            .replace(/\ufffd/g, "");
    }

    function readAsText(file: globalThis.File): Promise<string> {
        return new Promise((resolve, reject) => {
            const reader = new FileReader();
            reader.onload = () => resolve(reader.result as string);
            reader.onerror = reject;
            reader.readAsText(file);
        });
    }

    async function extractDocx(file: globalThis.File): Promise<string> {
        const mammoth = await import("mammoth");
        const arrayBuffer = await file.arrayBuffer();
        const result = await mammoth.extractRawText({ arrayBuffer });
        return fixEncoding(result.value);
    }

    async function importKnowledgeFiles(files: FileList | null) {
        if (!files || files.length === 0) return;
        knowledgeImportError = null;
        try {
            for (const file of Array.from(files)) {
                const ext = file.name.split(".").pop()?.toLowerCase() ?? "";
                let importedText = "";
                if (ext === "docx") {
                    importedText = await extractDocx(file);
                } else {
                    importedText = await readAsText(file);
                }
                importedText = importedText.trim();
                if (!importedText) continue;

                const header =
                    pageFormatDraft === "rich"
                        ? `<p><strong>Imported from ${escapeHtml(file.name)}</strong></p>`
                        : `Imported from ${file.name}`;
                const incoming =
                    pageFormatDraft === "rich"
                        ? `${header}${plainTextToRichHtml(importedText)}`
                        : `${header}\n\n${importedText}`;

                if (!pageBodyDraft.trim()) {
                    pageBodyDraft = incoming;
                } else if (pageFormatDraft === "rich") {
                    pageBodyDraft = `${pageBodyDraft}<hr>${incoming}`;
                } else {
                    pageBodyDraft = `${pageBodyDraft}\n\n---\n\n${incoming}`;
                }
            }
        } catch (error) {
            knowledgeImportError = `Import failed: ${String(error)}`;
        }
    }

    function onKnowledgeImportInputChange(event: Event) {
        const input = event.currentTarget as HTMLInputElement | null;
        if (!input) return;
        void importKnowledgeFiles(input.files);
        input.value = "";
    }

    async function deletePage(id: string) {
        if ($readOnly || !canEditScopedSubsystem) return;
        const p = knowledgePages.find((p) => p.id === id);
        knowledgePages = knowledgePages.filter((p) => p.id !== id);
        if (activePageId === id) activePageId = knowledgePages[0]?.id ?? null;
        if (p) addActivity(`Deleted knowledge page "${p.title}".`);
        await invoke("delete_subsystem_knowledge", { id });
    }

    // ── Artifacts ──────────────────────────────────────────────────────────────

    let artifacts: Artifact[] = [];
    let showAddArtifact = false;
    let newType: ArtifactType = "doc";
    let newTitle = "";
    let newLink = "";
    let newNotes = "";
    let githubToken = "";
    let githubOwner = "";
    let githubRepo = "";
    let githubMeta: Record<
        string,
        {
            kind: "repo" | "issue" | "pull" | "commit";
            label: string;
            state?: string;
            subtitle?: string;
        }
    > = {};
    let githubLoading = new Set<string>();
    let githubRepos: Array<{ owner: string; repo: string; full: string }> = [];
    let githubIssues: Array<{
        number: number;
        title: string;
        state: string;
        url: string;
        isPull: boolean;
    }> = [];
    let githubRepoLoading = false;
    let githubIssueLoading = false;
    let githubIssuePage = 1;
    let githubIssueHasNext = false;
    let githubIssueSearch = "";

    function inferArtifactKind(
        link: string,
        current: ArtifactType,
    ): ArtifactType {
        if (!link) return current;
        const lower = link.toLowerCase();
        if (lower.includes("github.com")) return "github";
        return current;
    }

    function inferGithubTitle(link: string): string {
        try {
            const url = new URL(link);
            if (!url.hostname.includes("github.com")) return "";
            const parts = url.pathname.split("/").filter(Boolean);
            if (parts.length < 2) return "";
            const owner = parts[0];
            const repo = parts[1];
            if (parts.length >= 4 && parts[2] === "issues") {
                return `${owner}/${repo}#${parts[3]}`;
            }
            if (parts.length >= 4 && parts[2] === "pull") {
                return `${owner}/${repo}#${parts[3]}`;
            }
            if (parts.length >= 4 && parts[2] === "commit") {
                return `${owner}/${repo}@${parts[3].slice(0, 7)}`;
            }
            return `${owner}/${repo}`;
        } catch {
            return "";
        }
    }

    function inferArtifactTitle(link: string): string {
        if (!link) return "";
        if (link.toLowerCase().includes("github.com"))
            return inferGithubTitle(link);
        return "";
    }

    function handleLinkInput(value: string) {
        const inferred = inferArtifactKind(value, newType);
        if (inferred !== newType) newType = inferred;
        if (!newTitle.trim()) {
            const title = inferArtifactTitle(value);
            if (title) newTitle = title;
        }
    }

    async function loadGithubSettings() {
        githubToken =
            (await invoke<string | null>("get_setting", {
                key: "integration.github.token",
                project_id: projectId,
            })) ?? "";
        githubOwner =
            (await invoke<string | null>("get_setting", {
                key: "integration.github.owner",
                project_id: projectId,
            })) ?? "";
        githubRepo =
            (await invoke<string | null>("get_setting", {
                key: "integration.github.repo",
                project_id: projectId,
            })) ?? "";
        await refreshGithubRepos();
        if (githubOwner && githubRepo) {
            await refreshGithubIssues(githubOwner, githubRepo);
        }
    }

    function parseGithubLink(link: string): {
        kind: "repo" | "issue" | "pull" | "commit";
        owner: string;
        repo: string;
        number?: string;
        sha?: string;
    } | null {
        try {
            const url = new URL(link);
            if (!url.hostname.includes("github.com")) return null;
            const parts = url.pathname.split("/").filter(Boolean);
            if (parts.length < 2) return null;
            const owner = parts[0];
            const repo = parts[1];
            if (parts.length >= 4 && parts[2] === "issues") {
                return { kind: "issue", owner, repo, number: parts[3] };
            }
            if (parts.length >= 4 && parts[2] === "pull") {
                return { kind: "pull", owner, repo, number: parts[3] };
            }
            if (parts.length >= 4 && parts[2] === "commit") {
                return { kind: "commit", owner, repo, sha: parts[3] };
            }
            return { kind: "repo", owner, repo };
        } catch {
            return null;
        }
    }

    async function fetchGithubMeta(artifact: Artifact, force = false) {
        if (artifact.kind !== "github" || !artifact.link) return;
        if (
            !force &&
            (githubMeta[artifact.id] || githubLoading.has(artifact.id))
        )
            return;

        const parsed = parseGithubLink(artifact.link);
        if (!parsed) return;

        githubLoading.add(artifact.id);
        const headers: Record<string, string> = {
            Accept: "application/vnd.github+json",
        };
        if (githubToken) headers.Authorization = `Bearer ${githubToken}`;

        try {
            if (parsed.kind === "repo") {
                const res = await fetch(
                    `https://api.github.com/repos/${parsed.owner}/${parsed.repo}`,
                    {
                        headers,
                    },
                );
                if (!res.ok)
                    throw new Error(`GitHub repo fetch failed (${res.status})`);
                const data = await res.json();
                githubMeta = {
                    ...githubMeta,
                    [artifact.id]: {
                        kind: "repo",
                        label:
                            data.full_name ?? `${parsed.owner}/${parsed.repo}`,
                        subtitle: data.description ?? "",
                    },
                };
            } else if (parsed.kind === "issue" || parsed.kind === "pull") {
                const res = await fetch(
                    `https://api.github.com/repos/${parsed.owner}/${parsed.repo}/issues/${parsed.number}`,
                    { headers },
                );
                if (!res.ok)
                    throw new Error(
                        `GitHub issue fetch failed (${res.status})`,
                    );
                const data = await res.json();
                githubMeta = {
                    ...githubMeta,
                    [artifact.id]: {
                        kind: parsed.kind,
                        label:
                            data.title ??
                            `${parsed.owner}/${parsed.repo}#${parsed.number}`,
                        state: data.state,
                        subtitle: `${parsed.owner}/${parsed.repo}#${parsed.number}`,
                    },
                };
            } else if (parsed.kind === "commit") {
                const res = await fetch(
                    `https://api.github.com/repos/${parsed.owner}/${parsed.repo}/commits/${parsed.sha}`,
                    { headers },
                );
                if (!res.ok)
                    throw new Error(
                        `GitHub commit fetch failed (${res.status})`,
                    );
                const data = await res.json();
                const message = data.commit?.message?.split("\n")[0] ?? "";
                githubMeta = {
                    ...githubMeta,
                    [artifact.id]: {
                        kind: "commit",
                        label:
                            message ||
                            `${parsed.owner}/${parsed.repo}@${parsed.sha?.slice(0, 7)}`,
                        subtitle: `${parsed.owner}/${parsed.repo}@${parsed.sha?.slice(0, 7)}`,
                    },
                };
            }
        } catch (e) {
            console.warn("GitHub metadata fetch failed", e);
        } finally {
            githubLoading.delete(artifact.id);
        }
    }

    async function refreshGithubRepos() {
        githubRepoLoading = true;
        githubRepos = [];
        try {
            const headers: Record<string, string> = {
                Accept: "application/vnd.github+json",
            };
            if (githubToken) headers.Authorization = `Bearer ${githubToken}`;
            let url =
                "https://api.github.com/user/repos?per_page=50&sort=updated";
            if (githubOwner) {
                url = `https://api.github.com/orgs/${githubOwner}/repos?per_page=50&sort=updated`;
            }
            const res = await fetch(url, { headers });
            if (!res.ok)
                throw new Error(`GitHub repo fetch failed (${res.status})`);
            const data = await res.json();
            githubRepos = (data as any[]).map((r) => ({
                owner: r.owner?.login ?? githubOwner,
                repo: r.name,
                full: r.full_name ?? `${r.owner?.login}/${r.name}`,
            }));
            if (!githubOwner && !githubRepo && githubRepos.length > 0) {
                const first = githubRepos[0];
                githubOwner = first.owner;
                githubRepo = first.repo;
                await refreshGithubIssues(first.owner, first.repo);
            }
        } catch (e) {
            console.warn("GitHub repo list failed", e);
        } finally {
            githubRepoLoading = false;
        }
    }

    async function refreshGithubIssues(owner: string, repo: string, page = 1) {
        githubIssueLoading = true;
        githubIssues = [];
        try {
            const headers: Record<string, string> = {
                Accept: "application/vnd.github+json",
            };
            if (githubToken) headers.Authorization = `Bearer ${githubToken}`;
            const perPage = 50;
            const res = await fetch(
                `https://api.github.com/repos/${owner}/${repo}/issues?state=all&per_page=${perPage}&page=${page}`,
                { headers },
            );
            if (!res.ok)
                throw new Error(`GitHub issue fetch failed (${res.status})`);
            const data = await res.json();
            githubIssues = (data as any[]).map((i) => ({
                number: i.number,
                title: i.title,
                state: i.state,
                url: i.html_url,
                isPull: Boolean(i.pull_request),
            }));
            githubIssuePage = page;
            githubIssueHasNext = githubIssues.length === perPage;
        } catch (e) {
            console.warn("GitHub issue list failed", e);
        } finally {
            githubIssueLoading = false;
        }
    }

    async function selectGithubRepo(full: string) {
        const [owner, repo] = full.split("/");
        if (!owner || !repo) return;
        githubOwner = owner;
        githubRepo = repo;
        await refreshGithubIssues(owner, repo, 1);
    }

    function selectGithubIssue(issue: {
        number: number;
        title: string;
        url: string;
        isPull: boolean;
    }) {
        newType = "github";
        newTitle = `#${issue.number} ${issue.title}`;
        newLink = issue.url;
    }

    function selectGithubRepoArtifact() {
        if (!githubOwner || !githubRepo) return;
        newType = "github";
        newTitle = `${githubOwner}/${githubRepo}`;
        newLink = `https://github.com/${githubOwner}/${githubRepo}`;
    }

    $: filteredGithubIssues = githubIssues.filter((i) => {
        const q = githubIssueSearch.trim().toLowerCase();
        if (!q) return true;
        return (
            i.title.toLowerCase().includes(q) || String(i.number).includes(q)
        );
    });

    async function refreshArtifacts(id: string) {
        artifacts = await invoke<Artifact[]>("list_subsystem_artifacts", {
            subsystemId: id,
        });
        for (const a of artifacts) {
            if (a.kind === "github") fetchGithubMeta(a);
        }
    }

    async function addArtifact() {
        if ($readOnly || !canEditScopedSubsystem) return;
        if (!newTitle.trim() && !newLink.trim()) return;
        const inferredKind = inferArtifactKind(newLink.trim(), newType);
        const inferredTitle = inferArtifactTitle(newLink.trim());
        const a: Artifact = {
            id: crypto.randomUUID(),
            subsystem_id: subsystemId,
            kind: inferredKind,
            title: newTitle.trim() || inferredTitle || newLink.trim(),
            link: newLink.trim(),
            notes: newNotes.trim(),
            created_at: new Date().toISOString(),
        };
        artifacts = [a, ...artifacts];
        newType = "doc";
        newTitle = "";
        newLink = "";
        newNotes = "";
        showAddArtifact = false;
        addActivity(`Added artifact "${a.title}" (${a.kind}).`);
        await invoke("upsert_subsystem_artifact", { artifact: a });
        if (a.kind === "github") fetchGithubMeta(a);
    }

    async function removeArtifact(id: string) {
        if ($readOnly || !canEditScopedSubsystem) return;
        const a = artifacts.find((a) => a.id === id);
        artifacts = artifacts.filter((a) => a.id !== id);
        if (a) addActivity(`Removed artifact "${a.title}".`);
        await invoke("delete_subsystem_artifact", { id });
    }

    // ── Activity feed ──────────────────────────────────────────────────────────

    let activity: ActivityEntry[] = [];

    async function refreshActivity(id: string) {
        activity = await invoke<ActivityEntry[]>("list_subsystem_activity", {
            subsystemId: id,
        });
    }

    async function addActivity(text: string) {
        if ($readOnly || !canEditScopedSubsystem) return;
        const entry: ActivityEntry = {
            id: crypto.randomUUID(),
            subsystem_id: subsystemId,
            text,
            created_at: new Date().toISOString(),
        };
        activity = [entry, ...activity].slice(0, 50); // cap at 50
        await invoke("add_subsystem_activity", { entry });
    }

    // ── Description persistence ────────────────────────────────────────────────

    async function commitDesc() {
        if ($readOnly || !canEditScopedSubsystem) return;
        subsystemDescription = descDraft;
        editingDesc = false;
        const node = $nodes.find((n) => n.id === subsystemId) ?? null;
        if (node) {
            await saveNode({
                ...node,
                description: subsystemDescription,
                modified_at: new Date().toISOString(),
            });
        }
        addActivity("Updated subsystem description.");
    }

    // ── Lifecycle ──────────────────────────────────────────────────────────────

    onMount(async () => {
        await loadProject(projectId);
        await loadGithubSettings();
        const node = $nodes.find((n) => n.id === subsystemId) ?? null;
        subsystemDescription = node?.description ?? "";
        await refreshArtifacts(subsystemId);
        await refreshKnowledge(subsystemId);
        await refreshActivity(subsystemId);
        if (knowledgePages.length > 0) activePageId = knowledgePages[0].id;
    });

    const GITHUB_REFRESH_MS = 120000;
    let githubTimer: number | null = null;

    function startGithubAutoRefresh() {
        if (githubTimer) return;
        githubTimer = window.setInterval(() => {
            if (activeTab !== "artifacts") return;
            for (const a of artifacts) {
                if (a.kind === "github") fetchGithubMeta(a, true);
            }
        }, GITHUB_REFRESH_MS);
    }

    $: startGithubAutoRefresh();

    onDestroy(() => {
        if (githubTimer) {
            clearInterval(githubTimer);
            githubTimer = null;
        }
    });

    // ── Helpers ────────────────────────────────────────────────────────────────

    function fmtDate(iso: string) {
        const d = new Date(iso);
        return d.toLocaleDateString(undefined, {
            month: "short",
            day: "numeric",
            year: "numeric",
        });
    }

    function priorityColor(p: string) {
        if (p === "shall") return "#ef4444";
        if (p === "should") return "#f59e0b";
        return "#8888a8";
    }

    function statusColor(s: string) {
        if (s === "approved") return "#22c55e";
        if (s === "obsolete") return "#ef4444";
        return "#8888a8";
    }
</script>

<!-- ═══════════════════════════════════════════════════════════════════════════
     LAYOUT
════════════════════════════════════════════════════════════════════════════ -->
<div class="ss-root">
    <!-- ── Header ──────────────────────────────────────────────────────────── -->
    <header class="ss-header">
        <div class="header-left">
            <a class="breadcrumb" href="/project/{projectId}/system"
                >System Overview</a
            >
            <span class="breadcrumb-sep">/</span>
            <span class="breadcrumb-current">{subsystemName}</span>
        </div>
        <a class="btn-ghost-sm" href="/project/{projectId}/system"
            >← Return to System</a
        >
    </header>

    <!-- ── Hero ────────────────────────────────────────────────────────────── -->
    <div class="ss-hero page-header">
        <div class="hero-meta">
            <span class="hero-badge">Subsystem</span>
        </div>
        <h1 class="hero-title">{subsystemName}</h1>

        <!-- Editable description -->
        {#if editingDesc}
            <div class="desc-edit-row">
                <textarea
                    class="desc-input"
                    rows="3"
                    bind:value={descDraft}
                    placeholder="Add a description for this subsystem…"
                    on:keydown={(e) => {
                        if (e.key === "Enter" && e.ctrlKey) commitDesc();
                    }}
                ></textarea>
                <div class="desc-actions">
                    <button
                        class="btn-ghost-sm"
                        on:click={() => {
                            editingDesc = false;
                        }}>Cancel</button
                    >
                    <button
                        class="btn-primary-sm"
                        on:click={commitDesc}
                        disabled={$readOnly || !canEditScopedSubsystem}
                    >
                        Save
                    </button>
                </div>
            </div>
        {:else}
            <button
                class="desc-display"
                on:click={() => {
                    if ($readOnly || !canEditScopedSubsystem) return;
                    descDraft = subsystemDescription;
                    editingDesc = true;
                }}
                disabled={$readOnly || !canEditScopedSubsystem}
                title="Click to edit description"
            >
                {subsystemDescription || "Click to add a description…"}
            </button>
        {/if}

        <!-- Stat chips -->
        <div class="hero-stats">
            <div class="stat-chip">
                <span class="stat-value">{allocatedReqs.length}</span>
                <span class="stat-label">Requirements</span>
            </div>
            <div class="stat-chip">
                <span class="stat-value">{knowledgePages.length}</span>
                <span class="stat-label">Knowledge Pages</span>
            </div>
            <div class="stat-chip">
                <span class="stat-value">{artifacts.length}</span>
                <span class="stat-label">Artifacts</span>
            </div>
        </div>
    </div>

    <!-- ── Tab bar ──────────────────────────────────────────────────────────── -->
    <nav class="tab-bar">
        <button
            class="tab-btn"
            class:active={activeTab === "overview"}
            on:click={() => (activeTab = "overview")}
        >
            Overview
        </button>
        <button
            class="tab-btn"
            class:active={activeTab === "board"}
            on:click={() => (activeTab = "board")}
        >
            Board
            {#if allocatedReqs.length > 0}<span class="tab-count"
                    >{allocatedReqs.length}</span
                >{/if}
        </button>
        <button
            class="tab-btn"
            class:active={activeTab === "requirements"}
            on:click={() => (activeTab = "requirements")}
        >
            Requirements
            {#if allocatedReqs.length > 0}<span class="tab-count"
                    >{allocatedReqs.length}</span
                >{/if}
        </button>
        <button
            class="tab-btn"
            class:active={activeTab === "knowledge"}
            on:click={() => (activeTab = "knowledge")}
        >
            Knowledge
            {#if knowledgePages.length > 0}<span class="tab-count"
                    >{knowledgePages.length}</span
                >{/if}
        </button>
        <button
            class="tab-btn"
            class:active={activeTab === "artifacts"}
            on:click={() => (activeTab = "artifacts")}
        >
            Artifacts
            {#if artifacts.length > 0}<span class="tab-count"
                    >{artifacts.length}</span
                >{/if}
        </button>
    </nav>

    <div class="ss-body">
        {#if activeTab === "board"}
            <div class="jira-shell">
                <div class="jira-header">
                    <div class="jira-header-copy">
                        <h2>Subsystem Board</h2>
                        <p>Track requirement progress for {subsystemName}.</p>
                    </div>
                    <div class="jira-header-stats">
                        <span class="jira-stat-chip"
                            >To Do {boardTodoReqs.length}</span
                        >
                        <span class="jira-stat-chip"
                            >In Progress {boardDoingReqs.length}</span
                        >
                        <span class="jira-stat-chip"
                            >Done {boardDoneReqs.length}</span
                        >
                    </div>
                </div>

                <div class="jira-board">
                    {#each BOARD_LANES as lane}
                        {@const laneReqs = boardReqsForLane(lane.value)}
                        <section class="jira-lane">
                            <header class="jira-lane-head">{lane.label}</header>
                            <div class="jira-lane-body">
                                {#if laneReqs.length === 0}
                                    <div class="jira-empty">
                                        No items in this column.
                                    </div>
                                {:else}
                                    {#each laneReqs as req (req.id)}
                                        {@const d = reqData(req)}
                                        {@const boardPriority =
                                            boardPriorityForNode(req)}
                                        {@const boardAssignee =
                                            boardAssigneeForNode(req)}
                                        {@const boardDueDate =
                                            boardDueDateForNode(req)}
                                        {@const boardEstimate =
                                            boardEstimateForNode(req)}
                                        {@const boardLabels =
                                            boardLabelsForNode(req)}
                                        <article class="jira-card">
                                            <div class="jira-card-top">
                                                <span class="req-id-pill"
                                                    >{d.req_id ?? "REQ-?"}</span
                                                >
                                                <select
                                                    class="jira-status-select"
                                                    value={boardLaneForNode(
                                                        req,
                                                    )}
                                                    on:change={(e) =>
                                                        onBoardLaneChange(
                                                            req,
                                                            e,
                                                        )}
                                                    disabled={$readOnly ||
                                                        !canEditNode(req)}
                                                >
                                                    {#each BOARD_STATUS_OPTIONS as statusOpt}
                                                        <option
                                                            value={statusOpt.value}
                                                            >{statusOpt.label}</option
                                                        >
                                                    {/each}
                                                </select>
                                            </div>

                                            <div class="jira-card-name">
                                                {req.name}
                                            </div>
                                            {#if d.text}
                                                <div class="jira-card-text">
                                                    {d.text}
                                                </div>
                                            {/if}

                                            <div class="jira-card-fields">
                                                <label class="jira-card-field">
                                                    <span>Assignee</span>
                                                    <input
                                                        class="jira-card-input"
                                                        value={boardAssignee}
                                                        placeholder="Name or email"
                                                        on:change={(e) =>
                                                            onBoardAssigneeChange(
                                                                req,
                                                                e,
                                                            )}
                                                        disabled={$readOnly ||
                                                            !canEditNode(req)}
                                                    />
                                                </label>
                                                <label class="jira-card-field">
                                                    <span>Priority</span>
                                                    <select
                                                        class="jira-card-input jira-card-select"
                                                        value={boardPriority}
                                                        on:change={(e) =>
                                                            onBoardPriorityChange(
                                                                req,
                                                                e,
                                                            )}
                                                        disabled={$readOnly ||
                                                            !canEditNode(req)}
                                                    >
                                                        {#each BOARD_PRIORITY_OPTIONS as priOpt}
                                                            <option
                                                                value={priOpt.value}
                                                                >{priOpt.label}</option
                                                            >
                                                        {/each}
                                                    </select>
                                                </label>
                                                <label class="jira-card-field">
                                                    <span>Due Date</span>
                                                    <input
                                                        class="jira-card-input"
                                                        type="date"
                                                        value={boardDueDate}
                                                        on:change={(e) =>
                                                            onBoardDueDateChange(
                                                                req,
                                                                e,
                                                            )}
                                                        disabled={$readOnly ||
                                                            !canEditNode(req)}
                                                    />
                                                </label>
                                                <label class="jira-card-field">
                                                    <span>Estimate</span>
                                                    <input
                                                        class="jira-card-input"
                                                        value={boardEstimate}
                                                        placeholder="e.g. 6h or 3 pts"
                                                        on:change={(e) =>
                                                            onBoardEstimateChange(
                                                                req,
                                                                e,
                                                            )}
                                                        disabled={$readOnly ||
                                                            !canEditNode(req)}
                                                    />
                                                </label>
                                                <label
                                                    class="jira-card-field jira-card-field-wide"
                                                >
                                                    <span>Labels</span>
                                                    <input
                                                        class="jira-card-input"
                                                        value={boardLabelsCsvForNode(
                                                            req,
                                                        )}
                                                        placeholder="risk, interface, hardware"
                                                        on:change={(e) =>
                                                            onBoardLabelsChange(
                                                                req,
                                                                e,
                                                            )}
                                                        disabled={$readOnly ||
                                                            !canEditNode(req)}
                                                    />
                                                </label>
                                            </div>

                                            <div class="jira-chip-row">
                                                {#if boardPriority !== "none"}
                                                    <span
                                                        class={`jira-chip jira-chip-priority ${boardPriority}`}
                                                        >{boardPriority.toUpperCase()}</span
                                                    >
                                                {/if}
                                                {#if boardAssignee}
                                                    <span class="jira-chip"
                                                        >Assignee: {boardAssignee}</span
                                                    >
                                                {/if}
                                                {#if boardDueDate}
                                                    <span class="jira-chip"
                                                        >Due: {boardDueDate}</span
                                                    >
                                                {/if}
                                                {#if boardEstimate}
                                                    <span class="jira-chip"
                                                        >Estimate: {boardEstimate}</span
                                                    >
                                                {/if}
                                                {#each boardLabels as label (label)}
                                                    <span class="jira-chip"
                                                        >#{label}</span
                                                    >
                                                {/each}
                                            </div>
                                        </article>
                                    {/each}
                                {/if}
                            </div>
                        </section>
                    {/each}
                </div>

                <div class="jira-lower">
                    <section class="jira-panel">
                        <div class="jira-panel-head">
                            <h3>Unallocated Backlog</h3>
                            <span>{unallocatedReqs.length}</span>
                        </div>
                        {#if unallocatedReqs.length === 0}
                            <div class="jira-empty">
                                No unallocated requirements available.
                            </div>
                        {:else}
                            <div class="jira-backlog-list">
                                {#each unallocatedReqs as req (req.id)}
                                    {@const d = reqData(req)}
                                    <div class="jira-backlog-item">
                                        <span class="req-id-pill"
                                            >{d.req_id ?? "REQ-?"}</span
                                        >
                                        <span class="jira-backlog-name"
                                            >{req.name}</span
                                        >
                                        <button
                                            class="btn-flowdown"
                                            on:click={() => flowDown(req)}
                                            disabled={$readOnly ||
                                                !canEditNode(req)}
                                        >
                                            Add
                                        </button>
                                    </div>
                                {/each}
                            </div>
                        {/if}
                    </section>

                    <section class="jira-panel">
                        <div class="jira-panel-head">
                            <h3>Recent Activity</h3>
                            <span>{activity.length}</span>
                        </div>
                        {#if activity.length === 0}
                            <div class="jira-empty">No activity yet.</div>
                        {:else}
                            <div class="activity-list">
                                {#each activity.slice(0, 10) as entry (entry.id)}
                                    <div class="activity-row">
                                        <span class="activity-dot"></span>
                                        <span class="activity-text"
                                            >{entry.text}</span
                                        >
                                        <span class="activity-date"
                                            >{fmtDate(entry.created_at)}</span
                                        >
                                    </div>
                                {/each}
                            </div>
                        {/if}
                    </section>
                </div>
            </div>
        {/if}

        <!-- ════ OVERVIEW ════ -->
        {#if activeTab === "overview"}
            <div class="overview-grid">
                <!-- Allocated reqs summary -->
                <section class="card">
                    <div class="card-header">
                        <h2>Allocated Requirements</h2>
                        <button
                            class="btn-link-sm"
                            on:click={() => (activeTab = "requirements")}
                            >View all →</button
                        >
                    </div>
                    {#if allocatedReqs.length === 0}
                        <div class="empty-hint">
                            No requirements flowed down yet. Go to the
                            Requirements tab to allocate.
                        </div>
                    {:else}
                        <div class="req-summary-list">
                            {#each allocatedReqs.slice(0, 5) as req (req.id)}
                                {@const d = reqData(req)}
                                <div class="req-summary-row">
                                    <span class="req-id-pill"
                                        >{d.req_id ?? "REQ-?"}</span
                                    >
                                    <span class="req-name-text">{req.name}</span
                                    >
                                    <span
                                        class="pill-mini"
                                        style="color:{priorityColor(
                                            d.priority,
                                        )}">{d.priority}</span
                                    >
                                    <span
                                        class="pill-mini"
                                        style="color:{statusColor(d.status)}"
                                        >{d.status}</span
                                    >
                                </div>
                            {/each}
                            {#if allocatedReqs.length > 5}
                                <div class="more-hint">
                                    +{allocatedReqs.length - 5} more
                                </div>
                            {/if}
                        </div>
                    {/if}
                </section>

                <!-- Recent knowledge -->
                <section class="card">
                    <div class="card-header">
                        <h2>Knowledge Base</h2>
                        <button
                            class="btn-link-sm"
                            on:click={() => (activeTab = "knowledge")}
                            >View all →</button
                        >
                    </div>
                    {#if knowledgePages.length === 0}
                        <div class="empty-hint">
                            No knowledge pages yet. Create one in the Knowledge
                            tab.
                        </div>
                    {:else}
                        <div class="kb-summary-list">
                            {#each knowledgePages.slice(0, 4) as p (p.id)}
                                <button
                                    class="kb-summary-row"
                                    on:click={() => {
                                        activeTab = "knowledge";
                                        activePageId = p.id;
                                    }}
                                >
                                    <span class="kb-icon">📄</span>
                                    <span class="kb-title">{p.title}</span>
                                    <span class="kb-date"
                                        >{fmtDate(p.updated_at)}</span
                                    >
                                </button>
                            {/each}
                        </div>
                    {/if}
                </section>

                <!-- Artifacts summary -->
                <section class="card">
                    <div class="card-header">
                        <h2>Artifacts</h2>
                        <button
                            class="btn-link-sm"
                            on:click={() => (activeTab = "artifacts")}
                            >View all →</button
                        >
                    </div>
                    {#if artifacts.length === 0}
                        <div class="empty-hint">No artifacts linked yet.</div>
                    {:else}
                        <div class="artifact-summary-list">
                            {#each artifacts.slice(0, 4) as a (a.id)}
                                <div class="artifact-summary-row">
                                    <span class="artifact-icon"
                                        >{TYPE_ICON[a.kind]}</span
                                    >
                                    <span class="artifact-title-text"
                                        >{a.title}</span
                                    >
                                </div>
                            {/each}
                        </div>
                    {/if}
                </section>

                <!-- Activity feed -->
                <section class="card activity-card">
                    <div class="card-header">
                        <h2>Activity</h2>
                    </div>
                    {#if activity.length === 0}
                        <div class="empty-hint">No activity yet.</div>
                    {:else}
                        <div class="activity-list">
                            {#each activity.slice(0, 10) as entry (entry.id)}
                                <div class="activity-row">
                                    <span class="activity-dot"></span>
                                    <span class="activity-text"
                                        >{entry.text}</span
                                    >
                                    <span class="activity-date"
                                        >{fmtDate(entry.created_at)}</span
                                    >
                                </div>
                            {/each}
                        </div>
                    {/if}
                </section>
            </div>
        {/if}

        <!-- ════ REQUIREMENTS ════ -->
        {#if activeTab === "requirements"}
            <div class="req-tab">
                <!-- Allocated section -->
                <div class="req-section-header">
                    <h2>Allocated to {subsystemName}</h2>
                    <span class="section-count"
                        >{allocatedReqs.length} requirement{allocatedReqs.length ===
                        1
                            ? ""
                            : "s"}</span
                    >
                </div>

                {#if allocatedReqs.length === 0}
                    <div class="empty-state-block">
                        No requirements have been flowed down to this subsystem
                        yet. Use the panel below to allocate system-level
                        requirements.
                    </div>
                {:else}
                    <div class="req-cards">
                        {#each allocatedReqs as req (req.id)}
                            {@const d = reqData(req)}
                            <div class="req-card allocated">
                                <div class="req-card-top">
                                    <span class="req-id-pill"
                                        >{d.req_id ?? "REQ-?"}</span
                                    >
                                    <div class="req-pills">
                                        <span
                                            class="pill-mini"
                                            style="color:{priorityColor(
                                                d.priority,
                                            )};border-color:{priorityColor(
                                                d.priority,
                                            )}40">{d.priority}</span
                                        >
                                        <span
                                            class="pill-mini"
                                            style="color:{statusColor(
                                                d.status,
                                            )};border-color:{statusColor(
                                                d.status,
                                            )}40">{d.status}</span
                                        >
                                        {#if d.verification_method}
                                            <span class="pill-mini"
                                                >{d.verification_method}</span
                                            >
                                        {/if}
                                    </div>
                                    <button
                                        class="btn-remove"
                                        on:click={() => removeAllocation(req)}
                                        disabled={$readOnly ||
                                            !canEditNode(req)}
                                        title="Remove allocation"
                                    >
                                        Remove
                                    </button>
                                </div>
                                <div class="req-card-name">{req.name}</div>
                                {#if d.text}
                                    <div class="req-card-text">{d.text}</div>
                                {/if}
                                {#if d.rationale}
                                    <div class="req-card-rationale">
                                        <strong>Rationale:</strong>
                                        {d.rationale}
                                    </div>
                                {/if}
                            </div>
                        {/each}
                    </div>
                {/if}

                <!-- System-level pool -->
                <div
                    class="req-section-header"
                    style="margin-top: var(--space-6)"
                >
                    <h2>System-Level Pool</h2>
                    <span class="section-count"
                        >{unallocatedReqs.length} unallocated</span
                    >
                </div>

                {#if allRequirements.length === 0}
                    <div class="empty-state-block">
                        No system-level requirements found. Create requirements
                        on the Requirements page first.
                    </div>
                {:else if unallocatedReqs.length === 0}
                    <div class="empty-state-block">
                        No unallocated requirements available. All requirements
                        have been allocated to a subsystem.
                    </div>
                {:else}
                    <div class="req-cards">
                        {#each unallocatedReqs as req (req.id)}
                            {@const d = reqData(req)}
                            <div class="req-card pool">
                                <div class="req-card-top">
                                    <span class="req-id-pill"
                                        >{d.req_id ?? "REQ-?"}</span
                                    >
                                    <div class="req-pills">
                                        <span
                                            class="pill-mini"
                                            style="color:{priorityColor(
                                                d.priority,
                                            )};border-color:{priorityColor(
                                                d.priority,
                                            )}40">{d.priority}</span
                                        >
                                        <span
                                            class="pill-mini"
                                            style="color:{statusColor(
                                                d.status,
                                            )};border-color:{statusColor(
                                                d.status,
                                            )}40">{d.status}</span
                                        >
                                    </div>
                                    <button
                                        class="btn-flowdown"
                                        on:click={() => flowDown(req)}
                                        disabled={$readOnly ||
                                            !canEditNode(req)}
                                    >
                                        ↓ Flow Down
                                    </button>
                                </div>
                                <div class="req-card-name">{req.name}</div>
                                {#if d.text}
                                    <div class="req-card-text">{d.text}</div>
                                {/if}
                            </div>
                        {/each}
                    </div>
                {/if}
            </div>
        {/if}

        <!-- ════ KNOWLEDGE ════ -->
        {#if activeTab === "knowledge"}
            <div class="kb-layout">
                <!-- Sidebar: page list -->
                <aside class="kb-sidebar">
                    <div class="kb-sidebar-header">
                        <span class="kb-sidebar-title">Pages</span>
                        <button
                            class="btn-icon-sm"
                            on:click={() => (showNewPageModal = true)}
                            disabled={$readOnly || !canEditScopedSubsystem}
                            title="New page"
                        >
                            +
                        </button>
                    </div>
                    {#if knowledgePages.length === 0}
                        <div class="kb-empty-list">No pages yet.</div>
                    {:else}
                        <div class="kb-page-list">
                            {#each knowledgePages as p (p.id)}
                                <button
                                    class="kb-page-item"
                                    class:active={activePageId === p.id}
                                    on:click={() => openPage(p)}
                                >
                                    <span class="kb-page-icon">📄</span>
                                    <span class="kb-page-name">{p.title}</span>
                                </button>
                            {/each}
                        </div>
                    {/if}
                </aside>

                <!-- Main: editor/viewer -->
                <div class="kb-main">
                    {#if activePage === null}
                        <div class="kb-no-page">
                            <div class="kb-no-page-icon">📝</div>
                            <div class="kb-no-page-text">
                                Select a page or create a new one
                            </div>
                            <button
                                class="btn-primary"
                                on:click={() => (showNewPageModal = true)}
                                disabled={$readOnly || !canEditScopedSubsystem}
                            >
                                Create Page
                            </button>
                        </div>
                    {:else if pageEditing}
                        <div class="kb-editor">
                            <input
                                class="kb-title-input"
                                bind:value={pageTitleDraft}
                                placeholder="Page title…"
                            />

                            <div class="kb-editor-toolbar">
                                <div class="kb-editor-toolbar-left">
                                    <select
                                        class="kb-select"
                                        value={pageFormatDraft}
                                        on:change={onKnowledgeFormatSelectChange}
                                    >
                                        {#each KB_FORMAT_OPTIONS as opt}
                                            <option value={opt.value}
                                                >{opt.label}</option
                                            >
                                        {/each}
                                    </select>

                                    <div class="kb-editor-view-switch">
                                        <button
                                            class="btn-chip"
                                            class:active={kbEditorView ===
                                                "write"}
                                            on:click={() =>
                                                (kbEditorView = "write")}
                                            type="button"
                                        >
                                            Write
                                        </button>
                                        <button
                                            class="btn-chip"
                                            class:active={kbEditorView ===
                                                "preview"}
                                            on:click={() =>
                                                (kbEditorView = "preview")}
                                            type="button"
                                            disabled={pageFormatDraft ===
                                                "plain"}
                                        >
                                            Preview
                                        </button>
                                        <button
                                            class="btn-chip"
                                            class:active={kbEditorView ===
                                                "split"}
                                            on:click={() =>
                                                (kbEditorView = "split")}
                                            type="button"
                                            disabled={pageFormatDraft ===
                                                "plain"}
                                        >
                                            Split
                                        </button>
                                    </div>
                                </div>

                                <div class="kb-editor-toolbar-right">
                                    <button
                                        class="btn-ghost-sm"
                                        type="button"
                                        on:click={() =>
                                            knowledgeImportInput?.click()}
                                    >
                                        Import File
                                    </button>
                                    <input
                                        bind:this={knowledgeImportInput}
                                        type="file"
                                        accept=".txt,.md,.docx,.tex,.csv,.json,.log"
                                        style="display:none"
                                        on:change={onKnowledgeImportInputChange}
                                    />
                                </div>
                            </div>

                            {#if pageFormatDraft === "rich"}
                                <div class="kb-rich-toolbar">
                                    <button
                                        type="button"
                                        class="btn-rich"
                                        on:click={() =>
                                            applyRichCommand("bold")}
                                        ><strong>B</strong></button
                                    >
                                    <button
                                        type="button"
                                        class="btn-rich"
                                        on:click={() =>
                                            applyRichCommand("italic")}
                                        ><em>I</em></button
                                    >
                                    <button
                                        type="button"
                                        class="btn-rich"
                                        on:click={() =>
                                            applyRichCommand("underline")}
                                        ><u>U</u></button
                                    >
                                    <button
                                        type="button"
                                        class="btn-rich"
                                        on:click={() =>
                                            applyRichCommand(
                                                "insertUnorderedList",
                                            )}>• List</button
                                    >
                                    <button
                                        type="button"
                                        class="btn-rich"
                                        on:click={() =>
                                            applyRichCommand(
                                                "insertOrderedList",
                                            )}>1. List</button
                                    >
                                    <button
                                        type="button"
                                        class="btn-rich"
                                        on:click={() =>
                                            applyRichCommand(
                                                "formatBlock",
                                                "blockquote",
                                            )}>Quote</button
                                    >
                                    <button
                                        type="button"
                                        class="btn-rich"
                                        on:click={() =>
                                            applyRichCommand(
                                                "formatBlock",
                                                "h2",
                                            )}>H2</button
                                    >
                                    <button
                                        type="button"
                                        class="btn-rich"
                                        on:click={() =>
                                            applyRichCommand("removeFormat")}
                                        >Clear</button
                                    >
                                </div>
                            {/if}

                            <div
                                class="kb-editor-body"
                                class:split={kbEditorView === "split" &&
                                    pageFormatDraft !== "plain"}
                            >
                                {#if kbEditorView !== "preview" || pageFormatDraft === "plain"}
                                    {#if pageFormatDraft === "rich"}
                                        <div
                                            class="kb-body-input kb-body-rich"
                                            contenteditable="true"
                                            bind:this={richEditorEl}
                                            bind:innerHTML={pageBodyDraft}
                                            on:input={syncRichEditorDraft}
                                            data-placeholder="Write rich notes, decisions, and rationale..."
                                        ></div>
                                    {:else}
                                        <textarea
                                            class="kb-body-input"
                                            bind:value={pageBodyDraft}
                                            placeholder={pageFormatDraft ===
                                            "markdown"
                                                ? "Write Markdown notes. Supports headings, lists, code blocks, links, and LaTeX via $...$ / $$...$$."
                                                : "Write documentation, decisions, notes, trade studies…"}
                                        ></textarea>
                                    {/if}
                                {/if}

                                {#if kbEditorView !== "write" && pageFormatDraft !== "plain"}
                                    <div class="kb-preview-panel">
                                        <div class="kb-preview-label">
                                            Preview
                                        </div>
                                        <div
                                            class="kb-body-display kb-rendered-body"
                                        >
                                            {#if pageBodyDraft.trim()}
                                                {@html draftRenderedBody}
                                            {:else}
                                                <div class="kb-empty-body">
                                                    Start typing to preview
                                                    formatted output.
                                                </div>
                                            {/if}
                                        </div>
                                    </div>
                                {/if}
                            </div>

                            {#if knowledgeImportError}
                                <div class="kb-import-error">
                                    {knowledgeImportError}
                                </div>
                            {/if}

                            <div class="kb-editor-actions">
                                <button
                                    class="btn-ghost"
                                    on:click={() => (pageEditing = false)}
                                    >Cancel</button
                                >
                                <button
                                    class="btn-danger-sm"
                                    on:click={() => {
                                        if (activePage) {
                                            deletePage(activePage.id);
                                            pageEditing = false;
                                        }
                                    }}
                                    disabled={$readOnly ||
                                        !canEditScopedSubsystem}
                                >
                                    Delete
                                </button>
                                <button
                                    class="btn-primary"
                                    on:click={savePage}
                                    disabled={$readOnly ||
                                        !canEditScopedSubsystem}
                                >
                                    Save
                                </button>
                            </div>
                        </div>
                    {:else}
                        <div class="kb-viewer">
                            <div class="kb-viewer-top">
                                <div>
                                    <h2 class="kb-viewer-title">
                                        {activePage.title}
                                    </h2>
                                    <div class="kb-viewer-meta">
                                        <span class="kb-viewer-date"
                                            >Last updated {fmtDate(
                                                activePage.updated_at,
                                            )}</span
                                        >
                                        <span class="kb-format-pill"
                                            >{normalizeKnowledgeFormat(
                                                activePage.body_format,
                                            )}</span
                                        >
                                    </div>
                                </div>
                                <div class="kb-viewer-actions">
                                    <button
                                        class="btn-ghost"
                                        on:click={startEditPage}
                                        disabled={$readOnly ||
                                            !canEditScopedSubsystem}
                                    >
                                        Edit
                                    </button>
                                    <button
                                        class="btn-danger-sm"
                                        on:click={() =>
                                            activePage &&
                                            deletePage(activePage.id)}
                                        disabled={$readOnly ||
                                            !canEditScopedSubsystem}
                                    >
                                        Delete
                                    </button>
                                </div>
                            </div>
                            <div class="kb-body-display">
                                {#if activePage.body.trim()}
                                    <div class="kb-rendered-body">
                                        {@html activePageRenderedBody}
                                    </div>
                                {:else}
                                    <div class="kb-empty-body">
                                        This page is empty. Click Edit to add
                                        content.
                                    </div>
                                {/if}
                            </div>
                        </div>
                    {/if}
                </div>
            </div>
        {/if}

        <!-- ════ ARTIFACTS ════ -->
        {#if activeTab === "artifacts"}
            <div class="artifacts-tab">
                <div class="artifacts-header">
                    <div>
                        <h2>Artifacts</h2>
                        <div class="artifacts-subtitle">
                            Linked documents, Jira tickets, tests, repositories,
                            and references
                        </div>
                    </div>
                    <button
                        class="btn-primary"
                        on:click={() => (showAddArtifact = true)}
                        disabled={$readOnly || !canEditScopedSubsystem}
                    >
                        + Add Artifact
                    </button>
                </div>

                {#if artifacts.length === 0}
                    <div class="empty-state-block">
                        No artifacts linked yet. Add links to documents, Jira
                        tickets, test evidence, or external resources.
                    </div>
                {:else}
                    <div class="artifact-grid">
                        {#each artifacts as a (a.id)}
                            <div class="artifact-card">
                                <div class="artifact-card-top">
                                    <span class="artifact-type-icon"
                                        >{TYPE_ICON[a.kind]}</span
                                    >
                                    <div class="artifact-card-info">
                                        <div class="artifact-card-title">
                                            {a.title}
                                        </div>
                                        <div class="artifact-card-type">
                                            {a.kind}
                                        </div>
                                    </div>
                                    <div class="artifact-actions">
                                        {#if a.link}
                                            <a
                                                class="artifact-open"
                                                href={a.link}
                                                target="_blank"
                                                rel="noreferrer">Open</a
                                            >
                                        {/if}
                                        <button
                                            class="btn-remove"
                                            on:click={() =>
                                                removeArtifact(a.id)}
                                            disabled={$readOnly ||
                                                !canEditScopedSubsystem}
                                        >
                                            Remove
                                        </button>
                                    </div>
                                </div>
                                {#if a.kind === "github" && githubMeta[a.id]}
                                    <div class="artifact-gh">
                                        <span class="gh-kind"
                                            >{githubMeta[a.id].kind}</span
                                        >
                                        <span class="gh-label"
                                            >{githubMeta[a.id].label}</span
                                        >
                                        {#if githubMeta[a.id].state}
                                            <span class="gh-state"
                                                >{githubMeta[a.id].state}</span
                                            >
                                        {/if}
                                        {#if githubMeta[a.id].subtitle}
                                            <span class="gh-sub"
                                                >{githubMeta[a.id]
                                                    .subtitle}</span
                                            >
                                        {/if}
                                    </div>
                                {/if}
                                {#if a.link}
                                    <a
                                        class="artifact-link"
                                        href={a.link}
                                        target="_blank"
                                        rel="noreferrer">{a.link}</a
                                    >
                                {/if}
                                {#if a.notes}
                                    <div class="artifact-notes">{a.notes}</div>
                                {/if}
                                <div class="artifact-date">
                                    Added {fmtDate(a.created_at)}
                                </div>
                            </div>
                        {/each}
                    </div>
                {/if}
            </div>
        {/if}
    </div>
</div>

<!-- ════ MODAL: Add Artifact ════ -->
{#if showAddArtifact}
    <div
        class="modal-backdrop"
        role="button"
        tabindex="0"
        aria-label="Close"
        on:click={() => (showAddArtifact = false)}
        on:keydown={(e) =>
            (e.key === "Enter" || e.key === " ") && (showAddArtifact = false)}
        transition:fade={{ duration: 150 }}
    >
        <div
            class="modal"
            on:click|stopPropagation
            role="dialog"
            aria-label="Add artifact"
            transition:fly={{ y: 10, duration: 180, easing: cubicOut }}
        >
            <h2>Add Artifact</h2>
            <label class="field-label">
                Type
                <select class="field" bind:value={newType}>
                    {#each TYPE_OPTIONS as opt}
                        <option value={opt.value}>{opt.label}</option>
                    {/each}
                </select>
            </label>
            <label class="field-label">
                Title
                <input
                    class="field"
                    placeholder="Artifact title"
                    bind:value={newTitle}
                />
            </label>
            <label class="field-label">
                Link (optional)
                <input
                    class="field"
                    placeholder="https://…"
                    bind:value={newLink}
                    on:input={(e) => handleLinkInput(e.currentTarget.value)}
                />
            </label>
            {#if newType === "github"}
                <div class="gh-picker">
                    <div class="gh-row">
                        <div class="gh-label">Repository</div>
                        {#if githubRepoLoading}
                            <div class="gh-muted">Loading repos…</div>
                        {:else if githubRepos.length === 0}
                            <div class="gh-muted">
                                No repos found (check token access)
                            </div>
                        {:else}
                            <select
                                class="field"
                                value={`${githubOwner}/${githubRepo}`}
                                on:change={(e) =>
                                    selectGithubRepo(e.currentTarget.value)}
                            >
                                {#each githubRepos as r (r.full)}
                                    <option value={r.full}>{r.full}</option>
                                {/each}
                            </select>
                            <button
                                class="btn-ghost-sm"
                                on:click={selectGithubRepoArtifact}
                                >Use repo as artifact</button
                            >
                        {/if}
                    </div>
                    <div class="gh-row">
                        <div class="gh-label">Issues / PRs</div>
                        {#if githubIssueLoading}
                            <div class="gh-muted">Loading issues…</div>
                        {:else if githubIssues.length === 0}
                            <div class="gh-muted">No issues found.</div>
                        {:else}
                            <input
                                class="field gh-search"
                                placeholder="Filter by title or #"
                                bind:value={githubIssueSearch}
                            />
                            <div class="gh-issues">
                                {#each filteredGithubIssues as i (i.url)}
                                    <button
                                        class="gh-issue"
                                        on:click={() => selectGithubIssue(i)}
                                    >
                                        <span class="gh-num">#{i.number}</span>
                                        <span class="gh-title">{i.title}</span>
                                        <span class="gh-state">{i.state}</span>
                                        {#if i.isPull}<span class="gh-kind"
                                                >PR</span
                                            >{/if}
                                    </button>
                                {/each}
                            </div>
                            <div class="gh-pager">
                                <button
                                    class="btn-ghost-sm"
                                    disabled={githubIssuePage <= 1}
                                    on:click={() =>
                                        refreshGithubIssues(
                                            githubOwner,
                                            githubRepo,
                                            githubIssuePage - 1,
                                        )}
                                >
                                    Prev
                                </button>
                                <span class="gh-page"
                                    >Page {githubIssuePage}</span
                                >
                                <button
                                    class="btn-ghost-sm"
                                    disabled={!githubIssueHasNext}
                                    on:click={() =>
                                        refreshGithubIssues(
                                            githubOwner,
                                            githubRepo,
                                            githubIssuePage + 1,
                                        )}
                                >
                                    Next
                                </button>
                            </div>
                        {/if}
                    </div>
                </div>
            {/if}
            <label class="field-label">
                Notes
                <textarea
                    class="field"
                    rows="3"
                    placeholder="Optional notes…"
                    bind:value={newNotes}
                ></textarea>
            </label>
            <div class="modal-actions">
                <button
                    class="btn-ghost"
                    on:click={() => (showAddArtifact = false)}>Cancel</button
                >
                <button
                    class="btn-primary"
                    on:click={addArtifact}
                    disabled={(!newTitle.trim() && !newLink.trim()) ||
                        $readOnly ||
                        !canEditScopedSubsystem}
                >
                    Add
                </button>
            </div>
        </div>
    </div>
{/if}

<!-- ════ MODAL: New Knowledge Page ════ -->
{#if showNewPageModal}
    <div
        class="modal-backdrop"
        role="button"
        tabindex="0"
        aria-label="Close"
        on:click={() => (showNewPageModal = false)}
        on:keydown={(e) =>
            (e.key === "Enter" || e.key === " ") && (showNewPageModal = false)}
        transition:fade={{ duration: 150 }}
    >
        <div
            class="modal"
            on:click|stopPropagation
            role="dialog"
            aria-label="New knowledge page"
            transition:fly={{ y: 10, duration: 180, easing: cubicOut }}
        >
            <h2>New Knowledge Page</h2>
            <label class="field-label">
                Title
                <input
                    class="field"
                    placeholder="e.g. Trade Study, Design Decision, ICD…"
                    bind:value={newPageTitle}
                />
            </label>
            <div class="modal-actions">
                <button
                    class="btn-ghost"
                    on:click={() => (showNewPageModal = false)}>Cancel</button
                >
                <button
                    class="btn-primary"
                    on:click={createPage}
                    disabled={!newPageTitle.trim() ||
                        $readOnly ||
                        !canEditScopedSubsystem}
                >
                    Create
                </button>
            </div>
        </div>
    </div>
{/if}

<style>
    /* ── Root ─────────────────────────────────────────────────────────────── */
    .ss-root {
        display: flex;
        flex-direction: column;
        height: 100%;
        background: var(--surface-base);
        overflow: hidden;
    }

    /* ── Header / Breadcrumb ──────────────────────────────────────────────── */
    .ss-header {
        display: flex;
        justify-content: space-between;
        align-items: center;
        padding: 6px var(--space-5);
        border-bottom: 1px solid var(--surface-border);
        background: var(--surface-raised);
        flex-shrink: 0;
    }

    .header-left {
        display: flex;
        align-items: center;
        gap: var(--space-2);
        font-size: var(--text-sm);
    }

    .breadcrumb {
        color: var(--text-muted);
        text-decoration: none;
        transition: color var(--transition-fast);
    }
    .breadcrumb:hover {
        color: var(--accent-hover);
    }

    .breadcrumb-sep {
        color: var(--text-muted);
    }

    .breadcrumb-current {
        color: var(--text-primary);
        font-weight: var(--weight-medium);
    }

    /* ── Hero ─────────────────────────────────────────────────────────────── */
    .ss-hero {
        display: grid;
        grid-template-columns: minmax(280px, 1fr) auto;
        grid-template-areas:
            "meta stats"
            "title stats"
            "desc stats";
        column-gap: var(--space-4);
        row-gap: 4px;
        align-items: start;
        padding: var(--space-3) var(--space-5) var(--space-2);
        border-bottom: 1px solid var(--surface-border);
        flex-shrink: 0;
    }

    .hero-meta {
        grid-area: meta;
        margin-bottom: 0;
    }

    .hero-badge {
        font-size: var(--text-xs);
        text-transform: uppercase;
        letter-spacing: 0.18em;
        color: var(--accent-hover);
        background: var(--accent-dim);
        padding: 2px 8px;
        border-radius: 999px;
        border: 1px solid var(--accent);
    }

    .hero-title {
        grid-area: title;
        font-size: clamp(24px, 1.8vw, 30px);
        font-weight: var(--weight-semibold);
        line-height: 1.12;
        margin: 0;
    }

    .desc-display {
        grid-area: desc;
        background: none;
        border: none;
        text-align: left;
        color: var(--text-secondary);
        font-size: var(--text-xs);
        cursor: text;
        padding: 2px 0;
        max-width: 740px;
        display: block;
        width: 100%;
        line-height: 1.45;
        font-family: var(--font-sans);
    }
    .desc-display:hover {
        color: var(--text-primary);
    }

    .desc-edit-row {
        grid-area: desc;
        display: flex;
        flex-direction: column;
        gap: var(--space-2);
        max-width: 740px;
    }

    .desc-input {
        background: var(--surface-overlay);
        border: 1px solid var(--accent);
        border-radius: var(--radius-md);
        color: var(--text-primary);
        font-family: var(--font-sans);
        font-size: var(--text-sm);
        padding: var(--space-2) var(--space-3);
        resize: vertical;
        line-height: 1.6;
    }
    .desc-input:focus {
        outline: none;
    }

    .desc-actions {
        display: flex;
        gap: var(--space-2);
        justify-content: flex-end;
    }

    .hero-stats {
        grid-area: stats;
        display: flex;
        justify-content: flex-end;
        align-items: center;
        gap: var(--space-2);
        margin-top: 0;
        flex-wrap: wrap;
    }

    .stat-chip {
        display: flex;
        align-items: baseline;
        gap: 4px;
        padding: 4px 8px;
        border: 1px solid var(--surface-border-subtle);
        border-radius: var(--radius-md);
        background: var(--surface-overlay);
    }

    .stat-value {
        font-size: var(--text-lg);
        font-weight: var(--weight-semibold);
        color: var(--text-primary);
        line-height: 1;
    }

    .stat-label {
        font-size: 11px;
        color: var(--text-muted);
        text-transform: uppercase;
        letter-spacing: 0.06em;
    }

    /* ── Jira board mode ─────────────────────────────────────────────────── */
    .jira-shell {
        display: flex;
        flex-direction: column;
        gap: var(--space-4);
        padding: var(--space-4);
        overflow: auto;
    }

    .jira-header {
        display: flex;
        align-items: center;
        justify-content: space-between;
        gap: var(--space-3);
    }

    .jira-header-copy h2 {
        margin: 0;
        font-size: var(--text-lg);
        font-weight: var(--weight-semibold);
    }

    .jira-header-copy p {
        margin: var(--space-1) 0 0;
        color: var(--text-muted);
        font-size: var(--text-sm);
    }

    .jira-header-stats {
        display: flex;
        gap: var(--space-2);
        flex-wrap: wrap;
    }

    .jira-stat-chip {
        font-size: var(--text-xs);
        padding: 4px 10px;
        border-radius: 999px;
        border: 1px solid var(--surface-border);
        color: var(--text-secondary);
        background: var(--surface-raised);
    }

    .jira-board {
        display: grid;
        grid-template-columns: repeat(3, minmax(0, 1fr));
        gap: var(--space-3);
    }

    .jira-lane {
        border: 1px solid var(--surface-border);
        border-radius: var(--radius-lg);
        background: var(--surface-raised);
        min-height: 260px;
        display: flex;
        flex-direction: column;
    }

    .jira-lane-head {
        padding: var(--space-3);
        border-bottom: 1px solid var(--surface-border);
        font-size: var(--text-sm);
        font-weight: var(--weight-semibold);
    }

    .jira-lane-body {
        padding: var(--space-3);
        display: flex;
        flex-direction: column;
        gap: var(--space-2);
        overflow: auto;
    }

    .jira-card {
        background: var(--surface-base);
        border: 1px solid var(--surface-border);
        border-radius: var(--radius-md);
        padding: var(--space-2);
        display: flex;
        flex-direction: column;
        gap: var(--space-2);
    }

    .jira-card-top {
        display: flex;
        align-items: center;
        justify-content: space-between;
        gap: var(--space-2);
    }

    .jira-card-name {
        font-size: var(--text-sm);
        font-weight: var(--weight-semibold);
        color: var(--text-primary);
    }

    .jira-card-text {
        font-size: var(--text-xs);
        color: var(--text-muted);
        line-height: 1.5;
    }

    .jira-status-select {
        font-size: var(--text-xs);
        border-radius: var(--radius-sm);
        border: 1px solid var(--surface-border);
        background: var(--surface-overlay);
        color: var(--text-secondary);
        padding: 2px 6px;
    }

    .jira-card-fields {
        display: grid;
        grid-template-columns: repeat(2, minmax(0, 1fr));
        gap: 6px;
    }

    .jira-card-field {
        display: flex;
        flex-direction: column;
        gap: 4px;
        min-width: 0;
    }

    .jira-card-field span {
        font-size: 10px;
        letter-spacing: 0.06em;
        text-transform: uppercase;
        color: var(--text-muted);
    }

    .jira-card-field-wide {
        grid-column: 1 / -1;
    }

    .jira-card-input {
        width: 100%;
        border: 1px solid var(--surface-border);
        border-radius: var(--radius-sm);
        background: var(--surface-overlay);
        color: var(--text-secondary);
        font-size: var(--text-xs);
        padding: 4px 6px;
        font-family: var(--font-sans);
    }
    .jira-card-input:focus {
        outline: none;
        border-color: var(--accent);
    }

    .jira-card-select {
        appearance: none;
    }

    .jira-chip-row {
        display: flex;
        flex-wrap: wrap;
        gap: 6px;
        margin-top: 2px;
    }

    .jira-chip {
        font-size: 10px;
        border: 1px solid var(--surface-border);
        border-radius: 999px;
        padding: 2px 8px;
        color: var(--text-muted);
        background: var(--surface-overlay);
        white-space: nowrap;
    }

    .jira-chip-priority.p0 {
        color: #fca5a5;
        border-color: #ef444455;
        background: #ef44441a;
    }
    .jira-chip-priority.p1 {
        color: #fdba74;
        border-color: #f9731655;
        background: #f973161a;
    }
    .jira-chip-priority.p2 {
        color: #fde68a;
        border-color: #eab30855;
        background: #eab3081a;
    }
    .jira-chip-priority.p3 {
        color: #86efac;
        border-color: #22c55e55;
        background: #22c55e1a;
    }

    .jira-lower {
        display: grid;
        grid-template-columns: 1.3fr 1fr;
        gap: var(--space-3);
    }

    .jira-panel {
        border: 1px solid var(--surface-border);
        border-radius: var(--radius-lg);
        background: var(--surface-raised);
        padding: var(--space-3);
        display: flex;
        flex-direction: column;
        gap: var(--space-2);
        min-height: 180px;
    }

    .jira-panel-head {
        display: flex;
        align-items: baseline;
        justify-content: space-between;
        gap: var(--space-2);
        padding-bottom: var(--space-2);
        border-bottom: 1px solid var(--surface-border);
    }

    .jira-panel-head h3 {
        margin: 0;
        font-size: var(--text-sm);
        font-weight: var(--weight-semibold);
    }

    .jira-panel-head span {
        font-size: var(--text-xs);
        color: var(--text-muted);
    }

    .jira-backlog-list {
        display: flex;
        flex-direction: column;
        gap: var(--space-2);
        overflow: auto;
    }

    .jira-backlog-item {
        display: flex;
        align-items: center;
        gap: var(--space-2);
        border: 1px solid var(--surface-border);
        border-radius: var(--radius-md);
        background: var(--surface-base);
        padding: var(--space-2);
    }

    .jira-backlog-name {
        flex: 1;
        font-size: var(--text-sm);
        color: var(--text-secondary);
        min-width: 0;
        overflow: hidden;
        text-overflow: ellipsis;
        white-space: nowrap;
    }

    .jira-empty {
        font-size: var(--text-sm);
        color: var(--text-muted);
        font-style: italic;
    }

    /* ── Tab bar ──────────────────────────────────────────────────────────── */
    .tab-bar {
        display: flex;
        gap: 0;
        border-bottom: 1px solid var(--surface-border);
        background: var(--surface-raised);
        padding: 0 var(--space-5);
        flex-shrink: 0;
    }

    .tab-btn {
        display: flex;
        align-items: center;
        gap: var(--space-2);
        padding: 8px 12px;
        background: none;
        border: none;
        border-bottom: 2px solid transparent;
        color: var(--text-muted);
        font-size: var(--text-xs);
        font-weight: var(--weight-medium);
        cursor: pointer;
        transition:
            color var(--transition-fast),
            border-color var(--transition-fast);
        margin-bottom: -1px;
    }

    .tab-btn:hover {
        color: var(--text-primary);
    }

    .tab-btn.active {
        color: var(--accent-hover);
        border-bottom-color: var(--accent);
    }

    .tab-count {
        background: var(--accent-dim);
        color: var(--accent-hover);
        font-size: 10px;
        padding: 1px 6px;
        border-radius: 999px;
        font-weight: var(--weight-semibold);
    }

    @media (max-width: 1200px) {
        .ss-hero {
            grid-template-columns: 1fr;
            grid-template-areas:
                "meta"
                "title"
                "desc"
                "stats";
            row-gap: var(--space-2);
        }

        .hero-stats {
            justify-content: flex-start;
        }
    }

    /* ── Body ─────────────────────────────────────────────────────────────── */
    .ss-body {
        flex: 1;
        overflow: auto;
    }

    /* ── Overview ─────────────────────────────────────────────────────────── */
    .overview-grid {
        display: grid;
        grid-template-columns: repeat(2, 1fr);
        gap: var(--space-4);
        padding: var(--space-4);
    }

    .activity-card {
        grid-column: 1 / -1;
    }

    .card {
        background: var(--surface-raised);
        border: 1px solid var(--surface-border);
        border-radius: var(--radius-xl);
        padding: var(--space-4);
        display: flex;
        flex-direction: column;
        gap: var(--space-3);
    }

    .card-header {
        display: flex;
        justify-content: space-between;
        align-items: center;
    }

    .card-header h2 {
        font-size: var(--text-base);
        font-weight: var(--weight-semibold);
        margin: 0;
    }

    .empty-hint {
        font-size: var(--text-sm);
        color: var(--text-muted);
        font-style: italic;
    }

    /* req summary */
    .req-summary-list {
        display: flex;
        flex-direction: column;
        gap: var(--space-2);
    }
    .req-summary-row {
        display: flex;
        align-items: center;
        gap: var(--space-2);
        font-size: var(--text-sm);
    }
    .req-name-text {
        flex: 1;
        color: var(--text-secondary);
        white-space: nowrap;
        overflow: hidden;
        text-overflow: ellipsis;
    }
    .more-hint {
        font-size: var(--text-xs);
        color: var(--text-muted);
        padding-left: var(--space-1);
    }

    /* kb summary */
    .kb-summary-list {
        display: flex;
        flex-direction: column;
        gap: var(--space-1);
    }
    .kb-summary-row {
        display: flex;
        align-items: center;
        gap: var(--space-2);
        background: none;
        border: none;
        padding: var(--space-2) var(--space-2);
        border-radius: var(--radius-md);
        cursor: pointer;
        text-align: left;
        transition: background var(--transition-fast);
    }
    .kb-summary-row:hover {
        background: var(--surface-hover);
    }
    .kb-icon {
        font-size: 14px;
    }
    .kb-title {
        flex: 1;
        font-size: var(--text-sm);
        color: var(--text-secondary);
    }
    .kb-date {
        font-size: var(--text-xs);
        color: var(--text-muted);
    }

    /* artifact summary */
    .artifact-summary-list {
        display: flex;
        flex-direction: column;
        gap: var(--space-2);
    }
    .artifact-summary-row {
        display: flex;
        align-items: center;
        gap: var(--space-2);
    }
    .artifact-icon {
        font-size: 14px;
    }
    .artifact-title-text {
        font-size: var(--text-sm);
        color: var(--text-secondary);
    }

    /* activity */
    .activity-list {
        display: flex;
        flex-direction: column;
        gap: var(--space-2);
    }
    .activity-row {
        display: flex;
        align-items: flex-start;
        gap: var(--space-2);
        font-size: var(--text-sm);
    }
    .activity-dot {
        width: 6px;
        height: 6px;
        border-radius: 50%;
        background: var(--accent);
        flex-shrink: 0;
        margin-top: 5px;
    }
    .activity-text {
        flex: 1;
        color: var(--text-secondary);
    }
    .activity-date {
        font-size: var(--text-xs);
        color: var(--text-muted);
        white-space: nowrap;
    }

    /* ── Requirements tab ─────────────────────────────────────────────────── */
    .req-tab {
        padding: var(--space-5) var(--space-6);
        max-width: 900px;
    }

    .req-section-header {
        display: flex;
        align-items: baseline;
        gap: var(--space-3);
        margin-bottom: var(--space-3);
    }

    .req-section-header h2 {
        font-size: var(--text-lg);
        font-weight: var(--weight-semibold);
        margin: 0;
    }

    .section-count {
        font-size: var(--text-sm);
        color: var(--text-muted);
    }

    .empty-state-block {
        padding: var(--space-5);
        border: 1px dashed var(--surface-border);
        border-radius: var(--radius-lg);
        color: var(--text-secondary);
        font-size: var(--text-sm);
        text-align: center;
    }

    .req-cards {
        display: flex;
        flex-direction: column;
        gap: var(--space-3);
    }

    .req-card {
        border-radius: var(--radius-lg);
        padding: var(--space-3) var(--space-4);
        display: flex;
        flex-direction: column;
        gap: var(--space-2);
        transition: border-color var(--transition-fast);
    }

    .req-card.allocated {
        background: var(--surface-raised);
        border: 1px solid var(--color-requirement);
        border-left: 3px solid var(--color-requirement);
    }

    .req-card.pool {
        background: var(--surface-overlay);
        border: 1px solid var(--surface-border);
    }

    .req-card-top {
        display: flex;
        align-items: center;
        gap: var(--space-2);
    }

    .req-id-pill {
        font-family: var(--font-mono);
        font-size: var(--text-xs);
        color: var(--color-requirement);
        background: var(--color-requirement-bg);
        padding: 2px 6px;
        border-radius: var(--radius-sm);
        flex-shrink: 0;
    }

    .req-pills {
        display: flex;
        gap: var(--space-1);
        flex: 1;
        flex-wrap: wrap;
    }

    .pill-mini {
        font-size: var(--text-xs);
        padding: 1px 6px;
        border-radius: 999px;
        border: 1px solid var(--surface-border);
        color: var(--text-muted);
    }

    .req-card-name {
        font-size: var(--text-sm);
        font-weight: var(--weight-semibold);
    }

    .req-card-text {
        font-size: var(--text-xs);
        color: var(--text-secondary);
        line-height: 1.6;
    }

    .req-card-rationale {
        font-size: var(--text-xs);
        color: var(--text-muted);
        font-style: italic;
    }

    .btn-flowdown {
        margin-left: auto;
        padding: var(--space-1) var(--space-3);
        background: var(--accent-dim);
        color: var(--accent-hover);
        border: 1px solid var(--accent);
        border-radius: var(--radius-md);
        font-size: var(--text-xs);
        font-weight: var(--weight-medium);
        cursor: pointer;
        white-space: nowrap;
        transition: background var(--transition-fast);
    }
    .btn-flowdown:hover {
        background: var(--accent);
        color: #fff;
    }

    .btn-remove {
        margin-left: auto;
        padding: var(--space-1) var(--space-2);
        background: none;
        border: 1px solid transparent;
        border-radius: var(--radius-md);
        color: var(--text-muted);
        font-size: var(--text-xs);
        cursor: pointer;
        transition: all var(--transition-fast);
    }
    .btn-remove:hover {
        color: var(--color-error);
        border-color: var(--color-error);
    }

    /* ── Knowledge tab ────────────────────────────────────────────────────── */
    .kb-layout {
        display: grid;
        grid-template-columns: 220px 1fr;
        height: 100%;
    }

    .kb-sidebar {
        border-right: 1px solid var(--surface-border);
        background: var(--surface-raised);
        display: flex;
        flex-direction: column;
        overflow: hidden;
    }

    .kb-sidebar-header {
        display: flex;
        justify-content: space-between;
        align-items: center;
        padding: var(--space-3) var(--space-3);
        border-bottom: 1px solid var(--surface-border);
    }

    .kb-sidebar-title {
        font-size: var(--text-xs);
        text-transform: uppercase;
        letter-spacing: 0.12em;
        color: var(--text-muted);
        font-weight: var(--weight-semibold);
    }

    .kb-empty-list {
        padding: var(--space-4);
        font-size: var(--text-sm);
        color: var(--text-muted);
        text-align: center;
    }

    .kb-page-list {
        overflow-y: auto;
        flex: 1;
    }

    .kb-page-item {
        display: flex;
        align-items: center;
        gap: var(--space-2);
        width: 100%;
        background: none;
        border: none;
        padding: var(--space-2) var(--space-3);
        cursor: pointer;
        text-align: left;
        transition: background var(--transition-fast);
    }
    .kb-page-item:hover {
        background: var(--surface-hover);
    }
    .kb-page-item.active {
        background: var(--accent-dim);
    }

    .kb-page-icon {
        font-size: 13px;
        flex-shrink: 0;
    }
    .kb-page-name {
        font-size: var(--text-sm);
        color: var(--text-secondary);
        white-space: nowrap;
        overflow: hidden;
        text-overflow: ellipsis;
    }
    .kb-page-item.active .kb-page-name {
        color: var(--accent-hover);
    }

    .kb-main {
        display: flex;
        flex-direction: column;
        overflow: auto;
    }

    .kb-no-page {
        display: flex;
        flex-direction: column;
        align-items: center;
        justify-content: center;
        gap: var(--space-4);
        height: 100%;
        color: var(--text-muted);
    }
    .kb-no-page-icon {
        font-size: 48px;
    }
    .kb-no-page-text {
        font-size: var(--text-base);
    }

    .kb-editor {
        display: flex;
        flex-direction: column;
        flex: 1;
        padding: var(--space-6);
        gap: var(--space-4);
    }

    .kb-title-input {
        background: none;
        border: none;
        border-bottom: 2px solid var(--surface-border);
        color: var(--text-primary);
        font-size: var(--text-2xl);
        font-weight: var(--weight-semibold);
        font-family: var(--font-sans);
        padding-bottom: var(--space-2);
        transition: border-color var(--transition-fast);
    }
    .kb-title-input:focus {
        outline: none;
        border-bottom-color: var(--accent);
    }

    .kb-body-input {
        flex: 1;
        background: none;
        border: 1px solid var(--surface-border);
        border-radius: var(--radius-md);
        color: var(--text-primary);
        font-size: var(--text-base);
        font-family: var(--font-sans);
        line-height: 1.6;
        resize: none;
        min-height: 360px;
        padding: var(--space-3);
        background: var(--surface-overlay);
    }
    .kb-body-input:focus {
        outline: none;
        border-color: var(--accent);
    }

    .kb-editor-toolbar {
        display: flex;
        justify-content: space-between;
        align-items: center;
        gap: var(--space-3);
        border-bottom: 1px solid var(--surface-border);
        padding-bottom: var(--space-2);
    }

    .kb-editor-toolbar-left {
        display: flex;
        align-items: center;
        gap: var(--space-2);
        flex-wrap: wrap;
    }

    .kb-editor-toolbar-right {
        display: flex;
        align-items: center;
        gap: var(--space-2);
    }

    .kb-select {
        background: var(--surface-overlay);
        border: 1px solid var(--surface-border);
        color: var(--text-primary);
        border-radius: var(--radius-sm);
        font-size: var(--text-xs);
        padding: 6px 8px;
    }
    .kb-select:focus {
        outline: none;
        border-color: var(--accent);
    }

    .kb-editor-view-switch {
        display: inline-flex;
        align-items: center;
        gap: 6px;
    }

    .btn-chip {
        border: 1px solid var(--surface-border);
        background: var(--surface-overlay);
        color: var(--text-secondary);
        border-radius: var(--radius-sm);
        padding: 5px 9px;
        font-size: var(--text-xs);
        cursor: pointer;
    }
    .btn-chip:hover:not(:disabled) {
        border-color: var(--surface-border-bright);
        color: var(--text-primary);
    }
    .btn-chip.active {
        border-color: var(--accent);
        color: var(--accent-hover);
        background: var(--accent-dim);
    }
    .btn-chip:disabled {
        opacity: 0.55;
        cursor: not-allowed;
    }

    .kb-rich-toolbar {
        display: flex;
        flex-wrap: wrap;
        gap: 6px;
        padding-bottom: var(--space-2);
    }

    .btn-rich {
        border: 1px solid var(--surface-border);
        background: var(--surface-overlay);
        color: var(--text-secondary);
        border-radius: var(--radius-sm);
        padding: 5px 9px;
        font-size: var(--text-xs);
        cursor: pointer;
        min-width: 42px;
    }
    .btn-rich:hover {
        border-color: var(--accent);
        color: var(--accent-hover);
    }

    .kb-editor-body {
        display: grid;
        grid-template-columns: 1fr;
        gap: var(--space-3);
        align-items: stretch;
    }
    .kb-editor-body.split {
        grid-template-columns: 1fr 1fr;
    }

    .kb-preview-panel {
        border: 1px solid var(--surface-border);
        border-radius: var(--radius-md);
        background: var(--surface-overlay);
        min-height: 360px;
        padding: var(--space-2) var(--space-3) var(--space-3);
        overflow: auto;
    }

    .kb-preview-label {
        font-size: var(--text-xs);
        text-transform: uppercase;
        letter-spacing: 0.08em;
        color: var(--text-muted);
        margin-bottom: var(--space-2);
    }

    .kb-body-rich {
        overflow: auto;
    }
    .kb-body-rich:empty::before {
        content: attr(data-placeholder);
        color: var(--text-muted);
    }

    .kb-import-error {
        border: 1px solid #ef444455;
        background: #ef44441f;
        color: #fca5a5;
        border-radius: var(--radius-md);
        padding: 8px 10px;
        font-size: var(--text-xs);
    }

    .kb-editor-actions {
        display: flex;
        gap: var(--space-2);
        justify-content: flex-end;
        border-top: 1px solid var(--surface-border);
        padding-top: var(--space-3);
    }

    .kb-viewer {
        flex: 1;
        display: flex;
        flex-direction: column;
        padding: var(--space-6);
        max-width: 760px;
    }

    .kb-viewer-top {
        display: flex;
        justify-content: space-between;
        align-items: flex-start;
        margin-bottom: var(--space-6);
        padding-bottom: var(--space-4);
        border-bottom: 1px solid var(--surface-border);
    }

    .kb-viewer-title {
        font-size: var(--text-2xl);
        font-weight: var(--weight-semibold);
        margin-bottom: var(--space-1);
    }

    .kb-viewer-date {
        font-size: var(--text-xs);
        color: var(--text-muted);
    }

    .kb-viewer-meta {
        display: flex;
        align-items: center;
        gap: var(--space-2);
        flex-wrap: wrap;
    }

    .kb-format-pill {
        font-size: 10px;
        text-transform: uppercase;
        letter-spacing: 0.08em;
        border: 1px solid var(--surface-border);
        border-radius: 999px;
        padding: 2px 8px;
        color: var(--text-muted);
        background: var(--surface-overlay);
    }

    .kb-viewer-actions {
        display: flex;
        gap: var(--space-2);
        flex-shrink: 0;
    }

    .kb-body-display {
        line-height: 1.8;
    }

    .kb-para {
        font-size: var(--text-base);
        color: var(--text-secondary);
        margin: 0;
    }

    .kb-para-gap {
        height: var(--space-3);
    }

    .kb-empty-body {
        font-size: var(--text-sm);
        color: var(--text-muted);
        font-style: italic;
        padding: var(--space-6) 0;
    }

    .kb-rendered-body :global(h1),
    .kb-rendered-body :global(h2),
    .kb-rendered-body :global(h3),
    .kb-rendered-body :global(h4) {
        margin: 0 0 var(--space-2);
        color: var(--text-primary);
        line-height: 1.25;
    }
    .kb-rendered-body :global(h1) {
        font-size: var(--text-2xl);
    }
    .kb-rendered-body :global(h2) {
        font-size: var(--text-xl);
    }
    .kb-rendered-body :global(h3) {
        font-size: var(--text-lg);
    }
    .kb-rendered-body :global(h4) {
        font-size: var(--text-base);
    }

    .kb-rendered-body :global(ul),
    .kb-rendered-body :global(ol) {
        margin: 0 0 var(--space-3) 1.25rem;
        color: var(--text-secondary);
    }

    .kb-rendered-body :global(li) {
        margin-bottom: 4px;
    }

    .kb-rendered-body :global(blockquote) {
        margin: 0 0 var(--space-3);
        padding: 8px 12px;
        border-left: 3px solid var(--accent);
        color: var(--text-secondary);
        background: var(--surface-raised);
        border-radius: 0 var(--radius-sm) var(--radius-sm) 0;
    }

    .kb-rendered-body :global(code) {
        font-family: var(--font-mono);
        font-size: 12px;
        border: 1px solid var(--surface-border);
        background: #0e1320;
        border-radius: var(--radius-sm);
        padding: 1px 4px;
    }

    .kb-rendered-body :global(pre) {
        margin: 0 0 var(--space-3);
        border: 1px solid var(--surface-border);
        background: #0b1220;
        border-radius: var(--radius-md);
        padding: 10px 12px;
        overflow: auto;
    }
    .kb-rendered-body :global(pre code) {
        border: none;
        padding: 0;
        background: transparent;
    }

    .kb-rendered-body :global(a) {
        color: var(--accent-hover);
        text-decoration: underline;
    }

    .kb-latex-inline {
        font-family: var(--font-mono);
        font-size: 12px;
        border: 1px solid #4f46e533;
        background: #4f46e51a;
        color: #c7d2fe;
        border-radius: var(--radius-sm);
        padding: 1px 6px;
    }

    .kb-latex-block {
        margin: 0 0 var(--space-3);
        border: 1px solid #0ea5e933;
        background: #0ea5e914;
    }

    /* ── Artifacts tab ────────────────────────────────────────────────────── */
    .artifacts-tab {
        padding: var(--space-5) var(--space-6);
    }

    .artifacts-header {
        display: flex;
        justify-content: space-between;
        align-items: flex-end;
        margin-bottom: var(--space-5);
    }

    .artifacts-header h2 {
        font-size: var(--text-xl);
        font-weight: var(--weight-semibold);
        margin-bottom: var(--space-1);
    }

    .artifacts-subtitle {
        font-size: var(--text-sm);
        color: var(--text-muted);
    }

    .artifact-grid {
        display: grid;
        grid-template-columns: repeat(auto-fill, minmax(280px, 1fr));
        gap: var(--space-3);
    }

    .artifact-card {
        background: var(--surface-raised);
        border: 1px solid var(--surface-border);
        border-radius: var(--radius-lg);
        padding: var(--space-4);
        display: flex;
        flex-direction: column;
        gap: var(--space-2);
        transition: border-color var(--transition-fast);
    }
    .artifact-card:hover {
        border-color: var(--accent);
    }

    .artifact-card-top {
        display: flex;
        align-items: flex-start;
        gap: var(--space-2);
    }

    .artifact-type-icon {
        font-size: 20px;
        flex-shrink: 0;
        margin-top: 1px;
    }

    .artifact-card-info {
        flex: 1;
    }

    .artifact-card-title {
        font-size: var(--text-sm);
        font-weight: var(--weight-semibold);
    }

    .artifact-card-type {
        font-size: var(--text-xs);
        text-transform: uppercase;
        letter-spacing: 0.08em;
        color: var(--text-muted);
    }

    .artifact-actions {
        display: flex;
        align-items: center;
        gap: var(--space-2);
    }

    .artifact-open {
        font-size: var(--text-xs);
        color: var(--accent-hover);
        text-decoration: none;
        padding: 2px 6px;
        border-radius: var(--radius-sm);
        border: 1px solid var(--surface-border);
    }
    .artifact-open:hover {
        background: var(--surface-hover);
    }

    .artifact-gh {
        display: flex;
        flex-wrap: wrap;
        gap: 6px;
        font-size: var(--text-xs);
        color: var(--text-secondary);
    }

    .gh-kind {
        text-transform: uppercase;
        letter-spacing: 0.06em;
        color: var(--text-muted);
    }

    .gh-label {
        font-weight: var(--weight-medium);
    }

    .gh-state {
        padding: 1px 6px;
        border-radius: 999px;
        border: 1px solid var(--surface-border);
        color: var(--text-secondary);
    }

    .gh-sub {
        color: var(--text-muted);
    }

    .artifact-link {
        font-size: var(--text-xs);
        color: var(--accent-hover);
        text-decoration: none;
        word-break: break-all;
    }
    .artifact-link:hover {
        text-decoration: underline;
    }

    .artifact-notes {
        font-size: var(--text-xs);
        color: var(--text-secondary);
    }

    .artifact-date {
        font-size: var(--text-xs);
        color: var(--text-muted);
        margin-top: auto;
        padding-top: var(--space-1);
    }

    /* ── Shared buttons ───────────────────────────────────────────────────── */
    .btn-primary {
        padding: var(--space-2) var(--space-4);
        background: var(--accent);
        color: #fff;
        border: none;
        border-radius: var(--radius-md);
        font-size: var(--text-sm);
        font-weight: var(--weight-medium);
        cursor: pointer;
        transition: background var(--transition-fast);
    }
    .btn-primary:hover:not(:disabled) {
        background: var(--accent-hover);
    }
    .btn-primary:disabled {
        opacity: 0.4;
        cursor: not-allowed;
    }

    .btn-primary-sm {
        padding: var(--space-1) var(--space-3);
        background: var(--accent);
        color: #fff;
        border: none;
        border-radius: var(--radius-md);
        font-size: var(--text-sm);
        cursor: pointer;
    }
    .btn-primary-sm:hover {
        background: var(--accent-hover);
    }

    .btn-ghost {
        padding: var(--space-2) var(--space-4);
        background: none;
        border: 1px solid var(--surface-border);
        border-radius: var(--radius-md);
        color: var(--text-secondary);
        font-size: var(--text-sm);
        cursor: pointer;
        transition: all var(--transition-fast);
    }
    .btn-ghost:hover {
        background: var(--surface-hover);
        color: var(--text-primary);
    }

    .btn-ghost-sm {
        padding: var(--space-1) var(--space-3);
        background: none;
        border: 1px solid var(--surface-border);
        border-radius: var(--radius-md);
        color: var(--text-secondary);
        font-size: var(--text-sm);
        text-decoration: none;
        cursor: pointer;
        transition: all var(--transition-fast);
    }
    .btn-ghost-sm:hover {
        background: var(--surface-hover);
        color: var(--text-primary);
    }

    .btn-danger-sm {
        padding: var(--space-1) var(--space-3);
        background: none;
        border: 1px solid var(--color-error);
        border-radius: var(--radius-md);
        color: var(--color-error);
        font-size: var(--text-sm);
        cursor: pointer;
        transition: all var(--transition-fast);
    }
    .btn-danger-sm:hover {
        background: var(--color-error);
        color: #fff;
    }

    .btn-link-sm {
        background: none;
        border: none;
        color: var(--accent-hover);
        font-size: var(--text-xs);
        cursor: pointer;
        padding: 0;
        text-decoration: none;
    }
    .btn-link-sm:hover {
        color: var(--text-primary);
    }

    .btn-icon-sm {
        width: 22px;
        height: 22px;
        background: none;
        border: 1px solid var(--surface-border);
        border-radius: var(--radius-sm);
        color: var(--text-muted);
        font-size: 16px;
        line-height: 1;
        cursor: pointer;
        display: flex;
        align-items: center;
        justify-content: center;
        transition: all var(--transition-fast);
    }
    .btn-icon-sm:hover {
        background: var(--surface-hover);
        color: var(--text-primary);
    }

    /* ── Modal ────────────────────────────────────────────────────────────── */
    .modal-backdrop {
        position: fixed;
        inset: 0;
        background: #00000080;
        display: flex;
        align-items: center;
        justify-content: center;
        z-index: var(--z-modal);
    }

    .modal {
        background: var(--surface-overlay);
        border: 1px solid var(--surface-border);
        border-radius: var(--radius-xl);
        padding: var(--space-6);
        width: 380px;
        display: flex;
        flex-direction: column;
        gap: var(--space-4);
        box-shadow: var(--shadow-lg);
    }

    .gh-picker {
        display: flex;
        flex-direction: column;
        gap: var(--space-3);
        border: 1px solid var(--surface-border);
        border-radius: var(--radius-md);
        padding: var(--space-3);
        background: var(--surface-raised);
    }

    .gh-row {
        display: flex;
        flex-direction: column;
        gap: var(--space-2);
    }

    .gh-label {
        font-size: var(--text-xs);
        text-transform: uppercase;
        letter-spacing: 0.08em;
        color: var(--text-muted);
    }

    .gh-muted {
        font-size: var(--text-xs);
        color: var(--text-secondary);
    }

    .gh-issues {
        display: flex;
        flex-direction: column;
        gap: 4px;
        max-height: 200px;
        overflow: auto;
        border: 1px solid var(--surface-border);
        border-radius: var(--radius-sm);
        padding: 4px;
        background: var(--surface-overlay);
    }

    .gh-search {
        font-size: var(--text-xs);
    }

    .gh-issue {
        display: grid;
        grid-template-columns: auto 1fr auto auto;
        gap: var(--space-2);
        align-items: center;
        padding: 4px 6px;
        background: none;
        border: none;
        text-align: left;
        cursor: pointer;
        border-radius: var(--radius-sm);
    }
    .gh-issue:hover {
        background: var(--surface-hover);
    }

    .gh-num {
        font-family: var(--font-mono);
        font-size: var(--text-xs);
        color: var(--text-muted);
    }

    .gh-title {
        font-size: var(--text-xs);
        color: var(--text-secondary);
        white-space: nowrap;
        overflow: hidden;
        text-overflow: ellipsis;
    }

    .gh-state {
        font-size: var(--text-xs);
        color: var(--text-muted);
    }

    .gh-kind {
        font-size: 10px;
        color: var(--accent-hover);
        border: 1px solid var(--accent);
        padding: 0 6px;
        border-radius: 999px;
    }

    .gh-pager {
        display: flex;
        align-items: center;
        gap: var(--space-2);
        justify-content: flex-end;
    }

    .gh-page {
        font-size: var(--text-xs);
        color: var(--text-muted);
    }

    .modal h2 {
        font-size: var(--text-lg);
        font-weight: var(--weight-semibold);
        margin: 0;
    }

    .field-label {
        display: flex;
        flex-direction: column;
        gap: var(--space-1);
        font-size: var(--text-sm);
        color: var(--text-secondary);
    }

    .field {
        width: 100%;
        background: var(--surface-raised);
        border: 1px solid var(--surface-border);
        border-radius: var(--radius-md);
        color: var(--text-primary);
        font-family: var(--font-sans);
        font-size: var(--text-base);
        padding: var(--space-2) var(--space-3);
        transition: border-color var(--transition-fast);
    }
    .field:focus {
        outline: none;
        border-color: var(--accent);
    }

    .modal-actions {
        display: flex;
        gap: var(--space-2);
        justify-content: flex-end;
    }

    /* ── Responsive ───────────────────────────────────────────────────────── */
    @media (max-width: 900px) {
        .jira-board {
            grid-template-columns: 1fr;
        }
        .jira-lower {
            grid-template-columns: 1fr;
        }
        .overview-grid {
            grid-template-columns: 1fr;
        }
        .activity-card {
            grid-column: 1;
        }
        .kb-layout {
            grid-template-columns: 1fr;
        }
        .kb-sidebar {
            border-right: none;
            border-bottom: 1px solid var(--surface-border);
        }
        .kb-editor-body.split {
            grid-template-columns: 1fr;
        }
        .kb-editor-toolbar {
            flex-direction: column;
            align-items: flex-start;
        }
        .jira-card-fields {
            grid-template-columns: 1fr;
        }
    }
</style>
