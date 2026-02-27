<script lang="ts">
    import { onDestroy, onMount, tick } from "svelte";
    import { fade, fly, slide } from "svelte/transition";
    import { page } from "$app/stores";
    import {
        loadProject,
        nodes,
        edges,
        saveNode,
        saveEdge,
        canCreateRequirementForAllocations,
        canCreateSubsystem,
    } from "$lib/store/model";
    import { invoke } from "@tauri-apps/api/core";
    import { v4 as uuidv4 } from "uuid";
    import type { Document, DocumentSection, Node } from "$lib/types";
    import {
        FileText,
        File as FileIcon,
        BookOpen,
        Upload,
        Inbox,
        CheckCircle,
        AlertTriangle,
        Loader,
        Link,
        Layers,
        List,
    } from "lucide-svelte";

    $: projectId = $page.params.id;

    // Types

    type Confidence = "high" | "medium" | "low";

    type ExtractedReq = {
        id: string;
        text: string; // full sentence (description)
        name: string; // short generated name for the block
        sentence: string; // full sentence (same as text, kept for compat)
        score: number; // 0.0-1.0
        confidence: Confidence;
        classification?: string;
        flags: string[];
        duplicate: boolean;
        imported: boolean;
        selected: boolean;
        allocation: string | null;
    };

    type ReviewMatch = {
        key: string;
        reqId: string;
        start: number;
        end: number;
        confidence: Confidence;
    };

    type ReviewSegment = {
        key: string;
        text: string;
        match: ReviewMatch | null;
    };

    type AllocationSuggestionInput = {
        id: string;
        sentence: string;
        name: string;
        confidence: Confidence;
        classification: string;
        flags: string[];
    };

    type AllocationSubsystemInput = {
        name: string;
        description: string;
    };

    type AllocationSuggestionOutput = {
        id?: string;
        sentence: string;
        allocation?: string;
        confidence?: Confidence;
        rationale?: string;
        new_subsystem_name?: string;
    };

    type AllocationHint = {
        source: "ai" | "heuristic";
        confidence: Confidence;
        rationale: string;
        newSubsystemName?: string;
    };

    type PendingSubsystemHint = {
        reqId: string;
        name: string;
        rationale: string;
        source: "ai" | "heuristic";
    };

    type ReviewHighlightCandidate = {
        reqId: string;
        confidence: Confidence;
        rx: RegExp;
    };

    type ExtractionCacheEntry = {
        signature: string;
        reqs: ExtractedReq[];
        parserMode: "python" | "heuristic";
        parserError: string;
        spacyAvailable: boolean;
        updatedAt: number;
    };

    type DocxPreviewModule = {
        renderAsync?: (
            data: ArrayBuffer,
            bodyContainer: HTMLElement,
            styleContainer?: HTMLElement,
            options?: Record<string, unknown>,
        ) => Promise<unknown>;
    };

    // State

    let documents: Document[] = [];
    let selectedDocId: string | null = null;
    let extractedReqs: ExtractedReq[] = [];
    let importing = false;
    let parsing = false;
    let analyzing = false; // python sidecar pass
    let parserMode: "python" | "heuristic" = "heuristic";
    let parserError = "";
    let spacyAvailable = false;
    let aiAvailable = false;
    let aiProviderId = "none";
    let aiProviderName = "AI"; // display name of active provider
    let useGraphRag = false;
    let aiLoading = false; // full-doc AI extraction running
    let aiError = "";
    let aiNotice = "";
    let permissionNotice = "";
    let allocationSuggestLoading = false;
    let allocationCreateLoading = false;
    let allocationSuggestError = "";
    let allocationSuggestNotice = "";
    let allocationHints = new Map<string, AllocationHint>();
    let pendingSubsystemHintReqCount = 0;
    let pendingSubsystemHintUniqueCount = 0;
    // Tracks names currently being created to prevent duplicate saves from
    // rapid clicks or concurrent calls before loadProject refreshes $nodes.
    const _subsystemCreationLock = new Set<string>();
    let dragOver = false;
    let fileInput: HTMLInputElement;
    let docsBodyEl: HTMLDivElement | null = null;
    let sidebarWidth = 300;
    let viewerHeight = 300;
    let resizingSidebar = false;
    let resizingViewer = false;
    let lastViewerResizeY = 0;
    let sidebarCollapsed = false;
    let sidebarCollapsedBeforeFocus = false;
    let focusMode = false;
    let reviewMode = false;
    let workspaceView: "viewer" | "requirements" | "split" = "split";
    let docSwitcherOpen = false;
    let docSwitcherQuery = "";
    let docSwitcherIndex = 0;
    let docSwitcherInput: HTMLInputElement | null = null;
    let docSwitcherShortcutLabel = "Ctrl+K";
    let docSwitcherItems: Document[] = [];
    let extractionCache = new Map<string, ExtractionCacheEntry>();
    let extractionRunToken = 0;
    let extractedReqsDocId: string | null = null;
    let reviewSelection = "";
    let reviewViewer: HTMLDivElement | null = null;
    let reviewTextSource = "";
    let reviewText = "";
    let reviewTextTruncated = false;
    let reviewMatches: ReviewMatch[] = [];
    let reviewSegments: ReviewSegment[] = [];
    let reviewMatchedRequirementIds = new Set<string>();
    let reviewUnmatchedCount = 0;
    let renderedDocHtml = "";
    let renderedPdfUrl: string | null = null;
    let renderedDocxBuffer: ArrayBuffer | null = null;
    let docxRenderHost: HTMLDivElement | null = null;
    let docxHtmlHost: HTMLDivElement | null = null;
    let plainTextHost: HTMLElement | null = null;
    let docxPreviewRendered = false;
    let docxPreviewAttempted = false;
    let docxRenderAttemptKey = "";
    let lastDocxPreviewError = "";
    let viewerRenderLoading = false;
    let viewerRenderError = "";
    let viewerRenderToken = 0;

    const MAX_REVIEW_TEXT_LENGTH = 120000;
    const MAX_MATCHES_PER_REQ = 8;
    const MIN_MANUAL_HIGHLIGHT_LEN = 12;
    const MIN_SIDEBAR_WIDTH = 220;
    const MAX_SIDEBAR_WIDTH = 520;
    const MIN_VIEWER_HEIGHT = 180;
    const DEFAULT_VIEWER_HEIGHT = 300;
    const SIDEBAR_WIDTH_STORAGE_KEY = "documents.sidebar.width.v1";
    const SIDEBAR_COLLAPSED_STORAGE_KEY = "documents.sidebar.collapsed.v1";
    const VIEWER_HEIGHT_STORAGE_KEY = "documents.viewer.height.v1";
    const TOKEN_STOP_WORDS = new Set([
        "the",
        "a",
        "an",
        "and",
        "or",
        "for",
        "to",
        "of",
        "in",
        "on",
        "by",
        "with",
        "from",
        "at",
        "as",
        "is",
        "are",
        "be",
        "shall",
        "must",
        "will",
        "should",
        "may",
        "system",
    ]);
    const NEW_SUBSYSTEM_HINTS: Array<{ name: string; keywords: string[] }> = [
        {
            name: "Sensor Subsystem",
            keywords: [
                "sensor",
                "camera",
                "eo",
                "ir",
                "lwir",
                "radar",
                "lidar",
            ],
        },
        {
            name: "Communications Subsystem",
            keywords: ["comm", "radio", "link", "network", "telemetry", "rf"],
        },
        {
            name: "Navigation Subsystem",
            keywords: [
                "gps",
                "navigation",
                "nav",
                "imu",
                "inertial",
                "position",
            ],
        },
        {
            name: "AI Processing Subsystem",
            keywords: [
                "ai",
                "ml",
                "inference",
                "vision",
                "algorithm",
                "accelerator",
            ],
        },
        {
            name: "Power Subsystem",
            keywords: ["power", "battery", "voltage", "current", "energy"],
        },
        {
            name: "Propulsion Subsystem",
            keywords: ["motor", "propulsion", "thrust", "rotor", "propeller"],
        },
        {
            name: "Ground Control Subsystem",
            keywords: ["gcs", "ground station", "control station"],
        },
    ];

    // Filter: which confidence tiers to show
    let showHigh = true;
    let showMedium = true;
    let showLow = false;
    let showDupes = false;

    $: visibleReqs = extractedReqs.filter((r) => {
        if (r.duplicate && !showDupes) return false;
        if (r.confidence === "high" && !showHigh) return false;
        if (r.confidence === "medium" && !showMedium) return false;
        if (r.confidence === "low" && !showLow) return false;
        return true;
    });
    $: selectedImportableCount = extractedReqs.filter(
        (r) => r.selected && !r.imported && canImportRequirement(r),
    ).length;

    $: selectedDoc = documents.find((d) => d.id === selectedDocId) ?? null;
    $: selectedDocFormat = selectedDoc
        ? detectFileFormat(selectedDoc)
        : "other";
    $: existingReqTexts = new Set(
        ($nodes.filter((n) => n.kind === "requirement") as Node[])
            .map((n) => normalizeKey((n.data as { text?: string }).text ?? ""))
            .filter(Boolean),
    );
    $: subsystems = $nodes.filter(
        (n) =>
            n.kind === "block" &&
            !(n.meta as Record<string, unknown>)?.system_root,
    );
    $: isOllamaProvider = aiProviderId === "ollama";
    $: aiRunLabel =
        isOllamaProvider && useGraphRag ? "Ollama + GraphRAG" : aiProviderName;
    $: effectiveSidebarWidth = sidebarCollapsed || focusMode ? 0 : sidebarWidth;
    $: showDocumentPanel = workspaceView !== "requirements";
    $: showRequirementsPanel = workspaceView !== "viewer";
    $: {
        const q = docSwitcherQuery.trim().toLowerCase();
        docSwitcherItems = documents.filter((doc) => {
            if (!q) return true;
            const name = doc.name.toLowerCase();
            const type = doc.doc_type.toLowerCase();
            return name.includes(q) || type.includes(q);
        });
    }
    $: if (docSwitcherOpen) {
        if (docSwitcherItems.length === 0) {
            docSwitcherIndex = 0;
        } else {
            docSwitcherIndex = clamp(
                docSwitcherIndex,
                0,
                docSwitcherItems.length - 1,
            );
        }
    }

    // Storage

    async function refreshDocuments() {
        documents = await invoke<Document[]>("list_documents", { projectId });
    }

    // File handling

    async function handleFiles(files: FileList | null) {
        if (!files || files.length === 0) return;
        for (const file of Array.from(files)) {
            await ingestFile(file);
        }
    }

    async function ingestFile(file: globalThis.File) {
        parsing = true;
        try {
            const ext = file.name.split(".").pop()?.toLowerCase() ?? "";
            const sourceArrayBuffer = await file.arrayBuffer();
            const sourceBase64 = arrayBufferToBase64(sourceArrayBuffer);
            const sourceMime = file.type || guessMimeType(ext);
            let text = "";

            if (ext === "txt" || ext === "md") {
                text = await readAsText(file);
            } else if (ext === "docx") {
                text = await extractDocx(file);
            } else if (ext === "pdf") {
                text = await extractPdf(file);
            } else {
                // Try plain text fallback
                text = await readAsText(file);
            }

            const doc: Document = {
                id: uuidv4(),
                project_id: projectId,
                name: file.name,
                doc_type: ext,
                size: file.size,
                added_at: new Date().toISOString(),
                text,
                source_base64: sourceBase64,
                source_mime: sourceMime,
            };

            await invoke("upsert_document", { doc });
            documents = [doc, ...documents];
            selectedDocId = doc.id;
            runExtraction(doc);
        } finally {
            parsing = false;
        }
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

    // Fix common Windows-1252 / mojibake artifacts that appear when
    // characters like degree symbols, dashes, and quotes can be double-encoded.
    function fixEncoding(text: string): string {
        return text
            .replace(/Â°/g, "\u00B0")
            .replace(/â€"/g, "-")
            .replace(/â€˜/g, "'")
            .replace(/â€™/g, "'")
            .replace(/â€œ/g, '"')
            .replace(/â€/g, '"')
            .replace(/Âµ/g, "\u00B5")
            .replace(/Â·/g, "\u00B7")
            .replace(/Â±/g, "\u00B1")
            .replace(/Ã—/g, "x")
            .replace(/Ã©/g, "e")
            .replace(/\u00AD/g, "") // soft hyphen
            .replace(/\uFFFD/g, ""); // replacement char
    }

    async function extractPdf(file: globalThis.File): Promise<string> {
        // Read as binary and extract visible text via simple byte scanning.
        // For a full PDF parse we'd need pdf.js - this covers most simple PDFs.
        const arrayBuffer = await file.arrayBuffer();
        const bytes = new Uint8Array(arrayBuffer);
        const raw = new TextDecoder("latin1").decode(bytes);

        // Extract text between BT...ET blocks (basic PDF text stream parsing)
        const lines: string[] = [];
        const btBlocks = raw.match(/BT[\s\S]*?ET/g) ?? [];
        for (const block of btBlocks) {
            const strings = block.match(/\(([^)]*)\)\s*Tj/g) ?? [];
            for (const s of strings) {
                const m = s.match(/\(([^)]*)\)/);
                if (m) lines.push(m[1]);
            }
        }

        // Also try to grab text from stream objects
        const streams = raw.match(/stream([\s\S]*?)endstream/g) ?? [];
        for (const stream of streams) {
            const printable = stream
                .replace(/[^\x20-\x7E\n\r]/g, " ")
                .replace(/\s+/g, " ");
            if (
                printable.includes("shall") ||
                printable.includes("must") ||
                printable.includes("will")
            ) {
                lines.push(printable);
            }
        }

        const text = lines.join(" ");
        return text.length > 100
            ? text
            : "[PDF text extraction limited - plain text content shown above. For best results use .docx or .txt]";
    }

    // Requirement extraction

    const SHALL_RE = /\b(shall|must|will)\b/i;
    const MODAL_RE =
        /\b(shall|must|should|will|required to|is required to|are required to|needs to|need to|is to|are to)\b/i;
    const NEGATION_RE = /\b(shall not|must not|should not|will not)\b/i;
    const CONSTRAINT_RE =
        /\b(minimum|maximum|at least|no less than|no more than|limit|threshold|within)\b/i;
    const UNIT_RE =
        /\b(\d+(\.\d+)?\s*(ms|s|sec|seconds|kg|g|m|mm|cm|km|hz|khz|mhz|ghz|v|a|w|db|\u00B0c|celsius|%|rpm|psi|pa))\b/i;
    const NUMBER_RE = /\b\d+(\.\d+)?\b/;
    const HEADING_POS_RE =
        /\b(requirements?|specifications?|performance|interface|safety|constraints?|compliance|environment)\b/i;
    const HEADING_NEG_RE =
        /\b(background|overview|scope|purpose|introduction)\b/i;

    type TextBlock = {
        text: string;
        sectionTitle: string;
        sectionRef: string;
        sectionType: "heading" | "paragraph" | "list_item";
        lineIndex: number;
    };

    type ParserBlock = {
        text: string;
        section_title: string;
        section_ref: string;
        section_type: "paragraph" | "list_item";
        line_index: number;
    };

    function normalizeText(text: string): string {
        return text.replace(/\s+/g, " ").trim();
    }

    function normalizeKey(text: string): string {
        return normalizeText(text).toLowerCase();
    }

    /**
     * Returns true when an AI-suggested name is too generic to be useful �
     * i.e. it could describe any requirement without change.
     * In those cases we keep the existing name rather than downgrading it.
     */
    function isGenericAiName(name: string | undefined): boolean {
        if (!name) return true;
        const lower = name.toLowerCase();
        // Ends with "requirement" or "requirements"
        if (/requirements?$/.test(lower)) return true;
        // Purely generic adjective + noun patterns
        const genericPhrases = [
            "system requirement",
            "functional requirement",
            "performance requirement",
            "security requirement",
            "data requirement",
            "interface requirement",
            "hardware requirement",
            "software requirement",
            "high priority requirement",
            "critical requirement",
            "general requirement",
            "user requirement",
        ];
        if (
            genericPhrases.some((p) => lower === p || lower.startsWith(p + " "))
        )
            return true;
        // Fewer than 2 meaningful words (single-word names are always generic)
        const words = name
            .trim()
            .split(/\s+/)
            .filter((w) => w.length > 1);
        if (words.length < 2) return true;
        return false;
    }

    /**
     * Rejects AI-suggested subsystem names that look like software functions
     * rather than physical/domain subsystems.
     * Valid: "Flight Controller", "Power Distribution Unit", "FPGA", "GPS Receiver"
     * Invalid: "display_search_results", "lock_account", "notify_emergency"
     */
    function isValidSubsystemName(name: string | undefined): boolean {
        if (!name || name.length < 3) return false;
        const lower = name.toLowerCase();
        // Snake_case or camelCase = software function name
        if (/[_]/.test(name)) return false;
        if (/^[a-z]/.test(name) && /[A-Z]/.test(name)) return false;
        // Verb-first patterns = software function ("display X", "lock X", "send X")
        if (
            /^(display|lock|unlock|send|get|set|fetch|load|save|update|delete|create|process|handle|manage|notify|alert|check|validate|compute|calculate|render|show|hide|enable|disable)\b/i.test(
                lower,
            )
        )
            return false;
        // Generic/vague names
        if (
            /^(new subsystem|subsystem|component|module|unit|system|feature|function|service)$/i.test(
                lower,
            )
        )
            return false;
        return true;
    }

    function escapeRegex(text: string): string {
        return text.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
    }

    function guessMimeType(ext: string): string {
        const lower = ext.toLowerCase();
        if (lower === "pdf") return "application/pdf";
        if (lower === "docx") {
            return "application/vnd.openxmlformats-officedocument.wordprocessingml.document";
        }
        if (lower === "md") return "text/markdown";
        if (lower === "txt") return "text/plain";
        return "application/octet-stream";
    }

    function detectFileFormat(
        doc: Document,
    ): "pdf" | "docx" | "text" | "other" {
        const mime = (doc.source_mime ?? "").toLowerCase();
        if (mime.includes("pdf")) return "pdf";
        if (mime.includes("wordprocessingml.document")) return "docx";

        const ext = doc.name.split(".").pop()?.toLowerCase() ?? "";
        if (ext === "pdf") return "pdf";
        if (ext === "docx") return "docx";
        if (ext === "txt" || ext === "md") return "text";
        return "other";
    }

    function arrayBufferToBase64(buffer: ArrayBuffer): string {
        const bytes = new Uint8Array(buffer);
        const chunkSize = 0x8000;
        let binary = "";
        for (let i = 0; i < bytes.length; i += chunkSize) {
            const chunk = bytes.subarray(i, i + chunkSize);
            binary += String.fromCharCode(...chunk);
        }
        return btoa(binary);
    }

    function base64ToArrayBuffer(base64: string): ArrayBuffer {
        const binary = atob(base64);
        const buffer = new ArrayBuffer(binary.length);
        const bytes = new Uint8Array(buffer);
        for (let i = 0; i < binary.length; i += 1) {
            bytes[i] = binary.charCodeAt(i);
        }
        return buffer;
    }

    function clearRenderedViewerContent() {
        renderedDocHtml = "";
        viewerRenderError = "";
        viewerRenderLoading = false;
        renderedDocxBuffer = null;
        docxHtmlHost = null;
        plainTextHost = null;
        docxPreviewRendered = false;
        docxPreviewAttempted = false;
        docxRenderAttemptKey = "";
        lastDocxPreviewError = "";
        if (docxRenderHost) {
            docxRenderHost.innerHTML = "";
        }
        if (renderedPdfUrl) {
            URL.revokeObjectURL(renderedPdfUrl);
            renderedPdfUrl = null;
        }
    }

    async function tryRenderDocxPreview(
        arrayBuffer: ArrayBuffer,
        token: number,
    ): Promise<boolean> {
        await tick();
        if (token !== viewerRenderToken) return false;
        if (!docxRenderHost || selectedDocFormat !== "docx") {
            return false;
        }

        const host = docxRenderHost;
        host.innerHTML = "";
        docxPreviewAttempted = true;

        try {
            const mod = (await import("docx-preview")) as DocxPreviewModule;
            if (typeof mod.renderAsync !== "function") {
                lastDocxPreviewError =
                    "docx-preview loaded but renderAsync export was missing";
                return false;
            }

            await mod.renderAsync(arrayBuffer.slice(0), host, undefined, {
                inWrapper: true,
                breakPages: true,
                ignoreWidth: false,
                ignoreHeight: false,
                useBase64URL: true,
                renderHeaders: true,
                renderFooters: true,
                renderFootnotes: true,
                renderEndnotes: true,
            });

            if (token !== viewerRenderToken) return false;
            docxPreviewRendered = true;
            lastDocxPreviewError = "";
            return true;
        } catch (error) {
            if (token !== viewerRenderToken) return false;
            host.innerHTML = "";
            docxPreviewRendered = false;
            lastDocxPreviewError = String(error);
            return false;
        }
    }

    async function renderDocumentWithFormatting(doc: Document | null) {
        const token = ++viewerRenderToken;
        clearRenderedViewerContent();
        if (!doc) return;

        const base64 = doc.source_base64?.trim() ?? "";
        if (!base64) {
            viewerRenderError =
                "Original file payload not available for this document. Re-upload to enable formatted preview.";
            return;
        }

        viewerRenderLoading = true;
        try {
            const fileFormat = detectFileFormat(doc);
            if (fileFormat === "pdf") {
                const arrayBuffer = base64ToArrayBuffer(base64);
                const mime = doc.source_mime || guessMimeType("pdf");
                const blob = new Blob([arrayBuffer], { type: mime });
                const url = URL.createObjectURL(blob);

                if (token !== viewerRenderToken) {
                    URL.revokeObjectURL(url);
                    return;
                }
                renderedPdfUrl = url;
            } else if (fileFormat === "docx") {
                const arrayBuffer = base64ToArrayBuffer(base64);
                renderedDocxBuffer = arrayBuffer;
                docxPreviewRendered = false;
                docxPreviewAttempted = false;
                renderedDocHtml = "";
                lastDocxPreviewError = "";

                const mammoth = await import("mammoth");
                const result = await mammoth.convertToHtml({ arrayBuffer });
                if (token !== viewerRenderToken) return;
                renderedDocHtml = result.value || "";
                viewerRenderError = "";
            } else {
                // Plain-text-like docs already render directly from extracted text.
                viewerRenderError = "";
            }
        } catch (error) {
            if (token !== viewerRenderToken) return;
            viewerRenderError = `Formatted preview failed: ${String(error)}`;
        } finally {
            if (token === viewerRenderToken) {
                viewerRenderLoading = false;
            }
        }
    }

    function clamp(n: number, min: number, max: number): number {
        return Math.max(min, Math.min(max, n));
    }

    function isEditableElement(target: EventTarget | null): boolean {
        const el = target as HTMLElement | null;
        if (!el) return false;
        const tag = el.tagName.toLowerCase();
        return (
            tag === "input" ||
            tag === "textarea" ||
            tag === "select" ||
            el.isContentEditable
        );
    }

    function toggleSidebarCollapsed() {
        sidebarCollapsed = !sidebarCollapsed;
    }

    function setFocusMode(next: boolean) {
        if (next === focusMode) return;
        if (next) {
            sidebarCollapsedBeforeFocus = sidebarCollapsed;
            focusMode = true;
            sidebarCollapsed = true;
            return;
        }
        focusMode = false;
        sidebarCollapsed = sidebarCollapsedBeforeFocus;
    }

    function toggleFocusMode() {
        setFocusMode(!focusMode);
    }

    async function openDocSwitcher() {
        if (documents.length === 0) return;
        docSwitcherQuery = "";
        const selectedIndex = documents.findIndex(
            (d) => d.id === selectedDocId,
        );
        docSwitcherIndex = selectedIndex >= 0 ? selectedIndex : 0;
        docSwitcherOpen = true;
        await tick();
        docSwitcherInput?.focus();
        docSwitcherInput?.select();
    }

    function closeDocSwitcher() {
        docSwitcherOpen = false;
        docSwitcherQuery = "";
    }

    function selectDocFromSwitcher(doc: Document) {
        selectedDocId = doc.id;
        closeDocSwitcher();
    }

    function handleGlobalKeydown(event: KeyboardEvent) {
        const key = event.key.toLowerCase();
        const hasMod = event.metaKey || event.ctrlKey;

        if (hasMod && key === "k") {
            event.preventDefault();
            void openDocSwitcher();
            return;
        }

        if (docSwitcherOpen) {
            if (event.key === "Escape") {
                event.preventDefault();
                closeDocSwitcher();
                return;
            }
            if (event.key === "ArrowDown") {
                event.preventDefault();
                if (docSwitcherItems.length > 0) {
                    docSwitcherIndex =
                        (docSwitcherIndex + 1) % docSwitcherItems.length;
                }
                return;
            }
            if (event.key === "ArrowUp") {
                event.preventDefault();
                if (docSwitcherItems.length > 0) {
                    docSwitcherIndex =
                        (docSwitcherIndex - 1 + docSwitcherItems.length) %
                        docSwitcherItems.length;
                }
                return;
            }
            if (event.key === "Enter") {
                if (docSwitcherItems.length > 0) {
                    event.preventDefault();
                    selectDocFromSwitcher(docSwitcherItems[docSwitcherIndex]);
                }
                return;
            }
            return;
        }

        if (isEditableElement(event.target)) return;

        if (event.key === "Escape" && focusMode) {
            event.preventDefault();
            setFocusMode(false);
        }
    }

    function startSidebarResize(event: MouseEvent) {
        if (sidebarCollapsed || focusMode) return;
        event.preventDefault();
        resizingSidebar = true;
    }

    function startViewerResize(event: MouseEvent) {
        event.preventDefault();
        resizingViewer = true;
        lastViewerResizeY = event.clientY;
    }

    function stopResizing() {
        if (!resizingSidebar && !resizingViewer) return;
        resizingSidebar = false;
        resizingViewer = false;
        if (typeof document !== "undefined") {
            document.body.style.cursor = "";
            document.body.style.userSelect = "";
        }
    }

    function onGlobalMouseMove(event: MouseEvent) {
        if (resizingSidebar && docsBodyEl) {
            const rect = docsBodyEl.getBoundingClientRect();
            const width = clamp(
                event.clientX - rect.left,
                MIN_SIDEBAR_WIDTH,
                MAX_SIDEBAR_WIDTH,
            );
            sidebarWidth = Math.round(width);
            if (typeof document !== "undefined") {
                document.body.style.cursor = "ew-resize";
                document.body.style.userSelect = "none";
            }
        }

        if (resizingViewer) {
            const delta = event.clientY - lastViewerResizeY;
            lastViewerResizeY = event.clientY;
            const maxHeight = clamp(
                Math.floor(
                    (docsBodyEl?.clientHeight ?? window.innerHeight) * 0.75,
                ),
                260,
                900,
            );
            viewerHeight = clamp(
                Math.round(viewerHeight + delta),
                MIN_VIEWER_HEIGHT,
                maxHeight,
            );
            if (typeof document !== "undefined") {
                document.body.style.cursor = "ns-resize";
                document.body.style.userSelect = "none";
            }
        }
    }

    function isHeading(line: string) {
        const num = line.match(/^(\d+(\.\d+)*)\s+(.{3,120})$/);
        if (num) return { ref: num[1], title: num[3].trim() };
        if (
            line.length <= 80 &&
            line === line.toUpperCase() &&
            /[A-Z]/.test(line)
        ) {
            return { ref: "", title: line.trim() };
        }
        if (line.endsWith(":") && line.length <= 80) {
            return { ref: "", title: line.replace(/:$/, "").trim() };
        }
        return null;
    }

    function isListItem(line: string) {
        const bullet = line.match(/^\s*[-*\u2022]\s+(.{3,})$/);
        if (bullet) return bullet[1].trim();
        const numbered = line.match(/^\s*\d+[\.\)]\s+(.{3,})$/);
        if (numbered) return numbered[1].trim();
        return null;
    }

    function buildBlocks(text: string): TextBlock[] {
        const lines = text.replace(/\r\n/g, "\n").split("\n");
        const blocks: TextBlock[] = [];
        let sectionTitle = "";
        let sectionRef = "";
        let paragraph = "";
        let paragraphStart = 0;

        const flushParagraph = (lineIndex: number) => {
            const cleaned = normalizeText(paragraph);
            if (cleaned.length > 0) {
                blocks.push({
                    text: cleaned,
                    sectionTitle,
                    sectionRef,
                    sectionType: "paragraph",
                    lineIndex,
                });
            }
            paragraph = "";
        };

        lines.forEach((raw, i) => {
            const line = raw.trim();
            if (!line) {
                flushParagraph(i);
                return;
            }

            const heading = isHeading(line);
            if (heading) {
                flushParagraph(i);
                sectionTitle = heading.title;
                sectionRef = heading.ref;
                blocks.push({
                    text: heading.title,
                    sectionTitle,
                    sectionRef,
                    sectionType: "heading",
                    lineIndex: i,
                });
                return;
            }

            const listItem = isListItem(line);
            if (listItem) {
                flushParagraph(i);
                blocks.push({
                    text: listItem,
                    sectionTitle,
                    sectionRef,
                    sectionType: "list_item",
                    lineIndex: i,
                });
                return;
            }

            if (!paragraph) paragraphStart = i;
            paragraph += (paragraph ? " " : "") + line;
            if (/[.!?]$/.test(line) || paragraph.length > 240) {
                flushParagraph(paragraphStart);
            }
        });

        flushParagraph(lines.length);
        return blocks.filter((b) => b.text.length > 10);
    }

    function scoreRequirement(block: TextBlock) {
        const text = block.text;
        let score = 0;
        const flags: string[] = [];

        if (MODAL_RE.test(text)) {
            score += 0.45;
            const modal = text.match(MODAL_RE)?.[0]?.toLowerCase() ?? "modal";
            flags.push(`modal:${modal}`);
        }

        if (NEGATION_RE.test(text)) {
            score += 0.08;
            flags.push("negation");
        }

        if (CONSTRAINT_RE.test(text)) {
            score += 0.08;
            flags.push("constraint");
        }

        if (UNIT_RE.test(text) || NUMBER_RE.test(text)) {
            score += 0.1;
            flags.push("numeric");
        }

        if (block.sectionTitle && HEADING_POS_RE.test(block.sectionTitle)) {
            score += 0.1;
            flags.push(`section:${block.sectionTitle}`);
        }

        if (block.sectionTitle && HEADING_NEG_RE.test(block.sectionTitle)) {
            score -= 0.15;
            flags.push("context:non-requirement");
        }

        if (block.sectionType === "list_item") {
            score += 0.05;
            flags.push("list-item");
        }

        if (text.length < 15) score -= 0.15;
        if (text.length > 320) score -= 0.1;
        if (text.endsWith(":")) score -= 0.2;

        score = Math.max(0, Math.min(1, score));
        let confidence: Confidence = "low";
        if (score >= 0.7) confidence = "high";
        else if (score >= 0.45) confidence = "medium";

        return { score, confidence, flags };
    }

    /** Quick client-side fallback name when sidecar is unavailable */
    function fallbackName(sentence: string): string {
        const m = SHALL_RE.exec(sentence);
        if (m) {
            const after = sentence
                .slice(m.index + m[0].length)
                .trim()
                .replace(/^(not\s+)?(be\s+)?(able\s+to\s+)?/i, "");
            const words = after.split(/\s+/).slice(0, 7);
            return words
                .join(" ")
                .replace(/[.,;:!?]+$/, "")
                .replace(/\b\w/g, (c) => c.toUpperCase());
        }
        return sentence
            .split(/\s+/)
            .slice(0, 7)
            .join(" ")
            .replace(/\b\w/g, (c) => c.toUpperCase());
    }

    function cloneExtractedReqs(reqs: ExtractedReq[]): ExtractedReq[] {
        return reqs.map((r) => ({ ...r, flags: [...r.flags] }));
    }

    function buildExtractionSignature(doc: Document): string {
        const text = doc.text ?? "";
        let hash = 2166136261;
        const step = Math.max(1, Math.floor(text.length / 2048));
        for (let i = 0; i < text.length; i += step) {
            hash ^= text.charCodeAt(i);
            hash = Math.imul(hash, 16777619);
        }
        return `${doc.doc_type ?? "General"}:${text.length}:${doc.size}:${hash >>> 0}`;
    }

    function writeExtractionCache(
        doc: Document,
        reqs: ExtractedReq[],
        mode: "python" | "heuristic",
        error: string,
        spacy: boolean,
    ) {
        extractionCache.set(doc.id, {
            signature: buildExtractionSignature(doc),
            reqs: cloneExtractedReqs(reqs),
            parserMode: mode,
            parserError: error,
            spacyAvailable: spacy,
            updatedAt: Date.now(),
        });
    }

    function applyCachedExtraction(entry: ExtractionCacheEntry, docId: string) {
        extractedReqs = cloneExtractedReqs(entry.reqs);
        extractedReqsDocId = docId;
        parserMode = entry.parserMode;
        parserError = entry.parserError;
        spacyAvailable = entry.spacyAvailable;
    }

    function hydrateReqListFromCache(doc: Document): boolean {
        const cached = extractionCache.get(doc.id);
        if (!cached) return false;
        if (cached.signature !== buildExtractionSignature(doc)) return false;
        applyCachedExtraction(cached, doc.id);
        return true;
    }

    async function runExtraction(doc: Document, force = false) {
        const token = ++extractionRunToken;
        aiError = "";
        aiNotice = "";
        permissionNotice = "";
        allocationSuggestError = "";
        allocationSuggestNotice = "";
        allocationHints = new Map();
        const signature = buildExtractionSignature(doc);
        const cached = extractionCache.get(doc.id);
        if (!force && cached && cached.signature === signature) {
            applyCachedExtraction(cached, doc.id);
            analyzing = false;
            clearReviewSelection();
            return;
        }

        analyzing = true;
        parserError = "";
        clearReviewSelection();

        const blocks = buildBlocks(doc.text);
        const parserBlocks: ParserBlock[] = blocks
            .filter((b) => b.sectionType !== "heading")
            .map((b) => ({
                text: b.text.trim(),
                section_title: b.sectionTitle ?? "",
                section_ref: b.sectionRef ?? "",
                section_type: b.sectionType as "paragraph" | "list_item",
                line_index: b.lineIndex,
            }))
            .filter((b) => b.text.length > 0);

        try {
            const raw = await invoke<string>("parse_requirements", {
                blocks: parserBlocks,
                docType: doc.doc_type ?? "General",
            });
            if (token !== extractionRunToken) return;
            const parsed = JSON.parse(raw) as {
                results?: Array<{
                    sentence: string;
                    name?: string;
                    score?: number;
                    confidence?: Confidence;
                    classification?: string;
                    flags?: string[];
                }>;
                spacy_available?: boolean;
                error?: string;
            };

            if (parsed.error) {
                throw new Error(parsed.error);
            }

            const results = parsed.results ?? [];
            spacyAvailable = Boolean(parsed.spacy_available);
            parserMode = "python";

            extractedReqs = results.map((r) => {
                const sentence = (r.sentence ?? "").trim();
                const key = normalizeKey(sentence);
                const confidence = r.confidence ?? "medium";
                const isDupe = existingReqTexts.has(key);
                return {
                    id: uuidv4(),
                    text: sentence,
                    name: r.name?.trim() || fallbackName(sentence),
                    sentence,
                    score: r.score ?? confidenceToScore(confidence),
                    confidence,
                    classification: r.classification ?? undefined,
                    flags: r.flags ?? [],
                    duplicate: isDupe,
                    imported: false,
                    selected: confidence === "high" && !isDupe,
                    allocation: null,
                };
            });
            extractedReqsDocId = doc.id;
            writeExtractionCache(
                doc,
                extractedReqs,
                parserMode,
                parserError,
                spacyAvailable,
            );
        } catch (e) {
            if (token !== extractionRunToken) return;
            parserMode = "heuristic";
            parserError = String(e);

            const candidates = blocks
                .filter((b) => b.sectionType !== "heading")
                .map((b) => {
                    const scored = scoreRequirement(b);
                    return { block: b, ...scored };
                })
                .filter((c) => c.score >= 0.35 || MODAL_RE.test(c.block.text));

            if (candidates.length === 0) {
                extractedReqs = [];
                extractedReqsDocId = doc.id;
                writeExtractionCache(
                    doc,
                    extractedReqs,
                    parserMode,
                    parserError,
                    spacyAvailable,
                );
                return;
            }

            extractedReqs = candidates.map((c) => {
                const key = normalizeKey(c.block.text);
                const isDupe = existingReqTexts.has(key);
                return {
                    id: uuidv4(),
                    text: c.block.text,
                    name: fallbackName(c.block.text),
                    sentence: c.block.text,
                    score: c.score,
                    confidence: c.confidence,
                    flags: c.flags,
                    duplicate: isDupe,
                    imported: false,
                    selected: c.confidence === "high" && !isDupe,
                    allocation: null,
                };
            });
            extractedReqsDocId = doc.id;
            writeExtractionCache(
                doc,
                extractedReqs,
                parserMode,
                parserError,
                spacyAvailable,
            );
        } finally {
            if (token === extractionRunToken) {
                analyzing = false;
            }
        }
    }

    function confidenceToScore(conf: Confidence): number {
        if (conf === "high") return 0.85;
        if (conf === "medium") return 0.6;
        return 0.35;
    }

    function buildSentenceRegex(sentence: string): RegExp | null {
        const cleaned = normalizeText(sentence);
        if (cleaned.length < MIN_MANUAL_HIGHLIGHT_LEN) return null;
        const pattern = escapeRegex(cleaned).replace(/\s+/g, "\\s+");
        return new RegExp(pattern, "gi");
    }

    function collectReviewMatches(
        text: string,
        reqs: ExtractedReq[],
    ): ReviewMatch[] {
        if (!text || reqs.length === 0) return [];

        const rawMatches: ReviewMatch[] = [];

        for (const req of reqs) {
            const sentence = req.sentence?.trim() || req.text?.trim();
            if (!sentence) continue;

            const rx = buildSentenceRegex(sentence);
            if (!rx) continue;

            let count = 0;
            let found: RegExpExecArray | null;
            while ((found = rx.exec(text)) && count < MAX_MATCHES_PER_REQ) {
                const hit = found[0] ?? "";
                if (!hit.trim()) {
                    if (rx.lastIndex === found.index) rx.lastIndex += 1;
                    continue;
                }

                const start = found.index;
                const end = start + hit.length;
                rawMatches.push({
                    key: `${req.id}:${start}:${end}`,
                    reqId: req.id,
                    start,
                    end,
                    confidence: req.confidence,
                });
                count += 1;

                if (rx.lastIndex === found.index) rx.lastIndex += 1;
            }
        }

        rawMatches.sort(
            (a, b) => a.start - b.start || b.end - b.start - (a.end - a.start),
        );

        const nonOverlapping: ReviewMatch[] = [];
        for (const match of rawMatches) {
            const overlaps = nonOverlapping.some(
                (existing) =>
                    match.start < existing.end && match.end > existing.start,
            );
            if (!overlaps) nonOverlapping.push(match);
        }

        nonOverlapping.sort((a, b) => a.start - b.start);
        return nonOverlapping;
    }

    function buildReviewSegments(
        text: string,
        matches: ReviewMatch[],
    ): ReviewSegment[] {
        if (!text) return [];
        if (matches.length === 0) {
            return [{ key: "plain:0", text, match: null }];
        }

        const segments: ReviewSegment[] = [];
        let cursor = 0;

        for (const match of matches) {
            if (match.start > cursor) {
                segments.push({
                    key: `plain:${cursor}`,
                    text: text.slice(cursor, match.start),
                    match: null,
                });
            }
            segments.push({
                key: `hit:${match.key}`,
                text: text.slice(match.start, match.end),
                match,
            });
            cursor = match.end;
        }

        if (cursor < text.length) {
            segments.push({
                key: `plain:${cursor}`,
                text: text.slice(cursor),
                match: null,
            });
        }

        return segments;
    }

    function clearRenderedRequirementHighlights(root: HTMLElement | null) {
        if (!root) return;
        const marks = root.querySelectorAll<HTMLElement>(
            "mark.req-highlight[data-req-id]",
        );
        for (const mark of Array.from(marks)) {
            const parent = mark.parentNode;
            if (!parent) continue;
            parent.replaceChild(
                document.createTextNode(mark.textContent ?? ""),
                mark,
            );
            parent.normalize();
        }
    }

    function buildRenderedHighlightCandidates(
        reqs: ExtractedReq[],
    ): ReviewHighlightCandidate[] {
        return reqs
            .map((req) => {
                const sentence = req.sentence?.trim() || req.text?.trim();
                if (!sentence) return null;
                const rx = buildSentenceRegex(sentence);
                if (!rx) return null;
                return {
                    reqId: req.id,
                    confidence: req.confidence,
                    rx: new RegExp(rx.source, "i"),
                } as ReviewHighlightCandidate;
            })
            .filter((c): c is ReviewHighlightCandidate => c !== null);
    }

    function findBestRenderedMatch(
        text: string,
        candidates: ReviewHighlightCandidate[],
    ): {
        start: number;
        end: number;
        text: string;
        candidate: ReviewHighlightCandidate;
    } | null {
        let best: {
            start: number;
            end: number;
            text: string;
            candidate: ReviewHighlightCandidate;
        } | null = null;

        for (const candidate of candidates) {
            candidate.rx.lastIndex = 0;
            const match = candidate.rx.exec(text);
            if (!match || typeof match.index !== "number") continue;
            const hit = match[0] ?? "";
            if (!hit.trim()) continue;

            const start = match.index;
            const end = start + hit.length;
            if (
                !best ||
                start < best.start ||
                (start === best.start && end - start > best.end - best.start)
            ) {
                best = {
                    start,
                    end,
                    text: hit,
                    candidate,
                };
            }
        }

        return best;
    }

    function applyRenderedHighlightsToTextNode(
        textNode: Text,
        candidates: ReviewHighlightCandidate[],
    ): number {
        let applied = 0;
        let current: Text | null = textNode;

        while (current) {
            const currentText = current.nodeValue ?? "";
            if (!currentText.trim()) break;
            const match = findBestRenderedMatch(currentText, candidates);
            if (!match) break;

            const parent = current.parentNode;
            if (!parent) break;

            const fragment = document.createDocumentFragment();
            const before = currentText.slice(0, match.start);
            const after = currentText.slice(match.end);

            if (before) {
                fragment.appendChild(document.createTextNode(before));
            }

            const mark = document.createElement("mark");
            mark.className = `req-highlight conf-${match.candidate.confidence}`;
            mark.dataset.reqId = match.candidate.reqId;
            mark.title = "Double-click to remove this requirement highlight";
            mark.textContent = match.text;
            fragment.appendChild(mark);

            let afterNode: Text | null = null;
            if (after) {
                afterNode = document.createTextNode(after);
                fragment.appendChild(afterNode);
            }

            parent.replaceChild(fragment, current);
            current = afterNode;
            applied += 1;
        }

        return applied;
    }

    function applyRenderedRequirementHighlights(
        root: HTMLElement,
        candidates: ReviewHighlightCandidate[],
    ): number {
        const walker = document.createTreeWalker(root, NodeFilter.SHOW_TEXT);
        const textNodes: Text[] = [];
        let node = walker.nextNode();
        while (node) {
            textNodes.push(node as Text);
            node = walker.nextNode();
        }

        let applied = 0;
        for (const textNode of textNodes) {
            const parent = textNode.parentElement;
            if (!parent || !parent.isConnected) continue;
            if (parent.closest("mark.req-highlight")) continue;
            if (parent.closest("script, style, noscript")) continue;
            applied += applyRenderedHighlightsToTextNode(textNode, candidates);
        }

        return applied;
    }

    function getRenderedHighlightRoots(): HTMLElement[] {
        const roots: HTMLElement[] = [];

        if (selectedDocFormat === "docx") {
            if (docxPreviewRendered && docxRenderHost?.isConnected) {
                roots.push(docxRenderHost);
            } else if (docxHtmlHost?.isConnected) {
                roots.push(docxHtmlHost);
            } else if (plainTextHost?.isConnected) {
                roots.push(plainTextHost);
            }
            return roots;
        }

        if (selectedDocFormat !== "pdf" && plainTextHost?.isConnected) {
            roots.push(plainTextHost);
        }

        return roots;
    }

    async function refreshRenderedRequirementHighlights() {
        await tick();
        if (typeof document === "undefined") return;
        if (!selectedDoc) return;

        const roots = getRenderedHighlightRoots();
        for (const root of roots) {
            clearRenderedRequirementHighlights(root);
        }
        if (!reviewMode || selectedDocFormat === "pdf" || roots.length === 0) {
            return;
        }

        const candidates = buildRenderedHighlightCandidates(extractedReqs);
        if (candidates.length === 0) return;

        for (const root of roots) {
            applyRenderedRequirementHighlights(root, candidates);
        }
    }

    function handleViewerDoubleClick(event: MouseEvent) {
        if (!reviewMode) return;
        const target = event.target as HTMLElement | null;
        const mark = target?.closest(
            "mark.req-highlight[data-req-id]",
        ) as HTMLElement | null;
        const reqId = mark?.dataset.reqId;
        if (!reqId) return;
        event.preventDefault();
        event.stopPropagation();
        removeExtractedRequirement(reqId);
    }

    function clearReviewSelection() {
        reviewSelection = "";
        if (typeof window === "undefined") return;
        window.getSelection()?.removeAllRanges();
    }

    function captureReviewSelection() {
        if (!reviewViewer || typeof window === "undefined") return;
        const selection = window.getSelection();
        if (!selection || selection.rangeCount === 0 || selection.isCollapsed) {
            reviewSelection = "";
            return;
        }

        const range = selection.getRangeAt(0);
        const container = range.commonAncestorContainer;
        const anchor =
            container.nodeType === Node.TEXT_NODE
                ? container.parentElement
                : (container as Element);

        if (!anchor || !reviewViewer.contains(anchor)) {
            reviewSelection = "";
            return;
        }

        reviewSelection = normalizeText(selection.toString());
    }

    async function runAIExtraction(doc: Document) {
        aiLoading = true;
        aiError = "";
        aiNotice = "";
        permissionNotice = "";
        clearReviewSelection();
        try {
            // Fast path: quality-review existing extracted requirements.
            if (extractedReqs.length > 0) {
                const reviewCandidates = extractedReqs.filter(
                    (r) =>
                        r.confidence !== "high" ||
                        !r.classification ||
                        r.classification === "unknown" ||
                        r.flags.some(
                            (f) =>
                                f.includes("compound") ||
                                f.includes("hedge") ||
                                f.includes("implicit_constraint"),
                        ),
                );
                const qualityInput = (
                    reviewCandidates.length > 0
                        ? reviewCandidates.slice(0, 40)
                        : extractedReqs.slice(0, 20)
                ).map((r) => ({
                    id: r.id,
                    sentence: r.sentence,
                    name: r.name,
                    confidence: r.confidence,
                    classification: r.classification ?? "",
                    flags: r.flags,
                    score: r.score,
                }));

                const reviewRaw = await invoke<string>(
                    "ai_quality_pass_requirements",
                    {
                        requirements: qualityInput,
                        docType: doc.doc_type ?? "General",
                        docName: doc.name,
                    },
                );

                const reviewed = JSON.parse(reviewRaw) as {
                    results?: Array<{
                        id?: string;
                        sentence: string;
                        name?: string;
                        confidence?: Confidence;
                        classification?: string;
                        flags?: string[];
                        review_priority?: "high" | "medium" | "low";
                    }>;
                };

                const updates = reviewed.results ?? [];
                if (updates.length === 0) {
                    aiNotice = `${aiProviderName} review completed with no suggested changes.`;
                    return;
                }

                const byId = new Map(
                    updates
                        .filter((u) => u.id && u.id.length > 0)
                        .map((u) => [u.id as string, u]),
                );
                const bySentence = new Map(
                    updates.map((u) => [normalizeKey(u.sentence), u]),
                );

                let changedCount = 0;
                extractedReqs = extractedReqs.map((req) => {
                    const update =
                        byId.get(req.id) ??
                        bySentence.get(normalizeKey(req.sentence));
                    if (!update) return req;

                    const confidence: Confidence =
                        update.confidence === "high" ||
                        update.confidence === "medium" ||
                        update.confidence === "low"
                            ? update.confidence
                            : req.confidence;
                    const isDupe = existingReqTexts.has(
                        normalizeKey(req.sentence),
                    );
                    const mergedFlags = Array.from(
                        new Set([
                            ...req.flags,
                            ...(update.flags ?? []),
                            "ai",
                            "quality-pass",
                            ...(update.review_priority === "high"
                                ? ["review:high"]
                                : []),
                        ]),
                    );
                    const nextName = isGenericAiName(update.name?.trim())
                        ? req.name
                        : update.name?.trim() || req.name;
                    const nextClassification =
                        update.classification?.trim() ||
                        req.classification ||
                        undefined;
                    const nextSelected =
                        req.imported || isDupe
                            ? false
                            : req.selected ||
                              confidence === "high" ||
                              update.review_priority === "high";
                    const nextScore = confidenceToScore(confidence);
                    const flagsChanged =
                        mergedFlags.length !== req.flags.length ||
                        mergedFlags.some((flag) => !req.flags.includes(flag));
                    if (
                        nextName !== req.name ||
                        confidence !== req.confidence ||
                        nextScore !== req.score ||
                        (nextClassification ?? "") !==
                            (req.classification ?? "") ||
                        nextSelected !== req.selected ||
                        flagsChanged
                    ) {
                        changedCount += 1;
                    }

                    return {
                        ...req,
                        name: nextName,
                        confidence,
                        score: nextScore,
                        classification: nextClassification,
                        flags: mergedFlags,
                        selected: nextSelected,
                    };
                });
                aiNotice =
                    changedCount > 0
                        ? `${aiProviderName} review applied ${changedCount} update${changedCount === 1 ? "" : "s"}.`
                        : `${aiProviderName} review completed with no suggested changes.`;
                extractedReqsDocId = doc.id;
                writeExtractionCache(
                    doc,
                    extractedReqs,
                    parserMode,
                    parserError,
                    spacyAvailable,
                );
                return;
            }

            // Slow fallback: full re-extraction when no parser results exist.
            const useGraphRagNow = aiProviderId === "ollama" && useGraphRag;
            const command = useGraphRagNow
                ? "graphrag_extract_requirements"
                : "ai_extract_requirements";
            const raw = await invoke<string>(command, {
                text: doc.text,
                docType: doc.doc_type ?? "General",
                docName: doc.name,
            });
            const parsed = JSON.parse(raw) as {
                results?: Array<{
                    sentence: string;
                    name?: string;
                    confidence?: Confidence;
                    classification?: string;
                    flags?: string[];
                }>;
            };
            const results = parsed.results ?? [];
            if (results.length === 0) {
                aiError = "AI found no requirements in this document.";
                return;
            }
            const importedByText = new Map(
                extractedReqs
                    .filter((r) => r.imported)
                    .map((r) => [normalizeKey(r.sentence), r]),
            );
            const modeFlags = useGraphRagNow ? ["ai", "graphrag"] : ["ai"];
            extractedReqs = results.map((r) => {
                const sentence = (r.sentence ?? "").trim();
                const key = normalizeKey(sentence);
                const existing = importedByText.get(key);
                const confidence = r.confidence ?? "medium";
                const isDupe = existingReqTexts.has(key);
                return {
                    id: existing?.id ?? uuidv4(),
                    text: sentence,
                    name: r.name?.trim() || fallbackName(sentence),
                    sentence,
                    score: confidenceToScore(confidence),
                    confidence,
                    classification: r.classification ?? undefined,
                    flags: Array.from(
                        new Set([...(r.flags ?? []), ...modeFlags]),
                    ),
                    duplicate: isDupe,
                    imported: existing?.imported ?? false,
                    selected:
                        confidence === "high" && !isDupe && !existing?.imported,
                    allocation: existing?.allocation ?? null,
                };
            });
            extractedReqsDocId = doc.id;
            writeExtractionCache(
                doc,
                extractedReqs,
                parserMode,
                parserError,
                spacyAvailable,
            );
        } catch (e) {
            const msg = String(e);
            aiError =
                msg === "no_api_key"
                    ? aiProviderId === "anthropic"
                        ? "No Anthropic API key configured - set ANTHROPIC_API_KEY and restart."
                        : "No AI provider is configured. Set one in Integrations."
                    : msg;
        } finally {
            aiLoading = false;
        }
    }

    // Import to requirements

    function nextReqId(): string {
        const existing = $nodes
            .filter((n) => n.kind === "requirement")
            .map((n) => {
                const id = (n.data as { req_id?: string }).req_id ?? "";
                const m = id.match(/(\d+)$/);
                return m ? parseInt(m[1], 10) : 0;
            });
        const max = existing.length ? Math.max(...existing) : 0;
        return `REQ-${String(max + 1).padStart(3, "0")}`;
    }

    function makeNode(req: ExtractedReq): Node {
        const now = new Date().toISOString();
        const aiGenerated = req.flags.some((f) => f === "ai");
        // priority derived from the modal keyword in the sentence
        const modal = /\bshall\b/i.test(req.text)
            ? "shall"
            : /\bmust\b/i.test(req.text)
              ? "shall"
              : "should";
        const allocations = req.allocation ? [req.allocation] : [];
        return {
            id: uuidv4(),
            project_id: projectId,
            kind: "requirement",
            name: req.name, // short generated name
            description: req.text, // full sentence goes to description
            data: {
                kind: "requirement",
                req_id: nextReqId(),
                text: req.text,
                rationale: `Extracted from: ${selectedDoc?.name ?? "document"}`,
                priority: modal,
                status: "draft",
                allocations: allocations,
                verification_method: undefined,
                source: selectedDoc?.name ?? "",
            },
            meta: {
                source_doc: selectedDoc?.name ?? "",
                actor: aiGenerated ? "ai" : "system",
                change_source: aiGenerated ? "ai" : "import",
            },
            created_at: now,
            modified_at: now,
        };
    }

    function canImportRequirement(req: ExtractedReq): boolean {
        return canCreateRequirementForAllocations(
            req.allocation ? [req.allocation] : undefined,
        );
    }

    async function importSelected() {
        permissionNotice = "";
        importing = true;
        const toImport = extractedReqs.filter((r) => r.selected && !r.imported);
        let importedCount = 0;
        let blockedCount = 0;
        for (const req of toImport) {
            if (!canImportRequirement(req)) {
                blockedCount += 1;
                continue;
            }
            await saveNode(makeNode(req));
            importedCount += 1;
        }
        extractedReqs = extractedReqs.map((r) =>
            r.selected && canImportRequirement(r)
                ? { ...r, imported: true, selected: false }
                : r,
        );
        if (blockedCount > 0) {
            permissionNotice = `Skipped ${blockedCount} selected requirement${blockedCount === 1 ? "" : "s"} due to access scope.`;
        } else if (importedCount > 0) {
            permissionNotice = "";
        }
        importing = false;
    }

    async function importOne(req: ExtractedReq) {
        if (req.imported) return;
        if (!canImportRequirement(req)) {
            permissionNotice =
                "You do not have permission to import this requirement allocation.";
            return;
        }
        permissionNotice = "";
        importing = true;
        await saveNode(makeNode(req));
        extractedReqs = extractedReqs.map((r) =>
            r.id === req.id ? { ...r, imported: true, selected: false } : r,
        );
        importing = false;
    }

    function removeExtractedRequirement(id: string) {
        const req = extractedReqs.find((r) => r.id === id);
        if (!req || req.imported) return;
        extractedReqs = extractedReqs.filter((r) => r.id !== id);
        setAllocationHint(id, null);
    }

    function addRequirementFromSelection() {
        const sentence = normalizeText(reviewSelection);
        if (sentence.length < MIN_MANUAL_HIGHLIGHT_LEN) return;

        const key = normalizeKey(sentence);
        const exists = extractedReqs.some(
            (r) => normalizeKey(r.sentence || r.text) === key,
        );
        if (exists) {
            clearReviewSelection();
            return;
        }

        const strongModal = /\b(shall|must)\b/i.test(sentence);
        const modal = MODAL_RE.test(sentence);
        const confidence: Confidence = strongModal
            ? "high"
            : modal
              ? "medium"
              : "low";
        const isDupe = existingReqTexts.has(key);

        extractedReqs = [
            {
                id: uuidv4(),
                text: sentence,
                name: fallbackName(sentence),
                sentence,
                score: confidenceToScore(confidence),
                confidence,
                classification: undefined,
                flags: modal
                    ? ["manual-highlight"]
                    : ["manual-highlight", "manual-review"],
                duplicate: isDupe,
                imported: false,
                selected: !isDupe && confidence !== "low",
                allocation: null,
            },
            ...extractedReqs,
        ];

        clearReviewSelection();
    }

    function toggleReviewMode() {
        reviewMode = !reviewMode;
        if (reviewMode && workspaceView === "requirements") {
            workspaceView = "viewer";
        }
        if (!reviewMode) clearReviewSelection();
    }

    function toggleSelect(id: string) {
        extractedReqs = extractedReqs.map((r) =>
            r.id === id ? { ...r, selected: !r.selected } : r,
        );
    }

    function selectAll() {
        extractedReqs = extractedReqs.map((r) => ({
            ...r,
            selected: !r.imported && visibleReqs.some((v) => v.id === r.id),
        }));
    }

    function selectNone() {
        extractedReqs = extractedReqs.map((r) => ({ ...r, selected: false }));
    }

    function setAllocation(id: string, value: string) {
        extractedReqs = extractedReqs.map((r) =>
            r.id === id ? { ...r, allocation: value || null } : r,
        );
        setAllocationHint(id, null);
    }

    function normalizeAllocation(
        value: string | null | undefined,
    ): string | null {
        const raw = (value ?? "").trim();
        if (!raw) return null;
        const norm = raw.toLowerCase();
        if (
            norm === "system" ||
            norm === "system level" ||
            norm === "system-level"
        ) {
            return null;
        }
        return raw;
    }

    function setAllocationHint(id: string, hint: AllocationHint | null) {
        const next = new Map(allocationHints);
        if (hint) next.set(id, hint);
        else next.delete(id);
        allocationHints = next;
    }

    function clearAllocationSuggestionState() {
        allocationSuggestLoading = false;
        allocationCreateLoading = false;
        allocationSuggestError = "";
        allocationSuggestNotice = "";
        allocationHints = new Map();
    }

    function allocationTokens(text: string): string[] {
        const raw = text
            .toLowerCase()
            .replace(/[^a-z0-9\s]/g, " ")
            .split(/\s+/)
            .map((t) => t.trim())
            .filter((t) => t.length > 1 && !TOKEN_STOP_WORDS.has(t));
        return Array.from(new Set(raw));
    }

    function getAllocationSubsystemInputs(): AllocationSubsystemInput[] {
        return subsystems
            .map((sub) => {
                const description = normalizeText(sub.description ?? "");
                return {
                    name: sub.name?.trim() ?? "",
                    description,
                };
            })
            .filter((sub) => sub.name.length > 0);
    }

    function inferNewSubsystemName(sentence: string): string {
        const lower = sentence.toLowerCase();
        for (const candidate of NEW_SUBSYSTEM_HINTS) {
            if (candidate.keywords.some((kw) => lower.includes(kw))) {
                return candidate.name;
            }
        }
        return "New Subsystem";
    }

    function heuristicAllocationSuggestion(
        req: ExtractedReq,
        subsystemInputs: AllocationSubsystemInput[],
    ): AllocationSuggestionOutput {
        const sentence = req.sentence || req.text;
        const lower = sentence.toLowerCase();

        const explicit = subsystemInputs.find((sub) =>
            lower.includes(sub.name.toLowerCase()),
        );
        if (explicit) {
            return {
                id: req.id,
                sentence,
                allocation: explicit.name,
                confidence: "high",
                rationale:
                    "Requirement explicitly references the subsystem name.",
            };
        }

        const reqTokens = allocationTokens(sentence);
        const classification = (req.classification ?? "").toLowerCase();
        let bestSubsystem: string | null = null;
        let bestScore = 0;

        for (const sub of subsystemInputs) {
            const subTokens = allocationTokens(
                `${sub.name} ${sub.description}`,
            );
            if (subTokens.length === 0 || reqTokens.length === 0) continue;
            const overlap = reqTokens.filter((token) =>
                subTokens.includes(token),
            );
            const score = overlap.length;
            if (score > bestScore) {
                bestScore = score;
                bestSubsystem = sub.name;
            }
        }

        if (bestSubsystem && bestScore >= 2) {
            return {
                id: req.id,
                sentence,
                allocation: bestSubsystem,
                confidence: bestScore >= 3 ? "high" : "medium",
                rationale:
                    bestScore >= 3
                        ? "Strong keyword overlap with subsystem context."
                        : "Moderate keyword overlap with subsystem context.",
            };
        }

        if (
            classification === "contractual" ||
            classification === "verification" ||
            classification === "interface"
        ) {
            return {
                id: req.id,
                sentence,
                allocation: "System Level",
                confidence: "medium",
                rationale:
                    "Requirement appears cross-cutting or verification-focused.",
            };
        }

        const newSubsystemName = inferNewSubsystemName(sentence);
        if (newSubsystemName !== "New Subsystem") {
            return {
                id: req.id,
                sentence,
                allocation: "System Level",
                confidence: "medium",
                rationale:
                    "Requirement looks subsystem-specific but does not match existing subsystem names.",
                new_subsystem_name: newSubsystemName,
            };
        }

        return {
            id: req.id,
            sentence,
            allocation: "System Level",
            confidence: "low",
            rationale:
                "No strong subsystem signal detected; kept at system level.",
        };
    }

    function allocationTargets(
        scope: "all" | "selected" | "single",
        reqId?: string,
    ): ExtractedReq[] {
        if (scope === "single" && reqId) {
            return extractedReqs.filter((r) => r.id === reqId && !r.imported);
        }
        if (scope === "selected") {
            return extractedReqs.filter((r) => r.selected && !r.imported);
        }
        return extractedReqs.filter((r) => !r.imported);
    }

    function pendingSubsystemHints(): PendingSubsystemHint[] {
        const out: PendingSubsystemHint[] = [];
        for (const req of extractedReqs) {
            if (req.imported) continue;
            const hint = allocationHints.get(req.id);
            const name = hint?.newSubsystemName?.trim() ?? "";
            if (!name) continue;
            out.push({
                reqId: req.id,
                name,
                rationale: hint?.rationale ?? "",
                source: hint?.source ?? "heuristic",
            });
        }
        return out;
    }

    function applyAllocationSuggestions(
        suggestions: Map<string, AllocationSuggestionOutput>,
        source: "ai" | "heuristic",
    ) {
        let assignedToSubsystem = 0;
        let systemLevelCount = 0;
        let newSubsystemCount = 0;
        const nextHints = new Map(allocationHints);

        extractedReqs = extractedReqs.map((req) => {
            const suggestion = suggestions.get(req.id);
            if (!suggestion || req.imported) return req;

            const normalized = normalizeAllocation(suggestion.allocation);
            if (normalized) assignedToSubsystem += 1;
            else systemLevelCount += 1;

            const newSubsystemName = isValidSubsystemName(
                suggestion.new_subsystem_name?.trim(),
            )
                ? suggestion.new_subsystem_name!.trim()
                : undefined;
            if (newSubsystemName) newSubsystemCount += 1;

            const hint: AllocationHint = {
                source,
                confidence: suggestion.confidence ?? "medium",
                rationale: suggestion.rationale?.trim() || "",
                newSubsystemName,
            };
            nextHints.set(req.id, hint);

            const allocFlag = normalized
                ? `alloc:${normalized.toLowerCase().replace(/\s+/g, "_")}`
                : "alloc:system_level";
            const extraFlags = newSubsystemName
                ? [
                      `suggest:new_subsystem:${newSubsystemName.toLowerCase().replace(/\s+/g, "_")}`,
                  ]
                : [];

            return {
                ...req,
                allocation: normalized,
                flags: Array.from(
                    new Set([
                        ...req.flags.filter(
                            (f) =>
                                !f.startsWith("alloc:") &&
                                !f.startsWith("suggest:new_subsystem:"),
                        ),
                        allocFlag,
                        ...extraFlags,
                    ]),
                ),
            };
        });
        allocationHints = nextHints;

        allocationSuggestNotice =
            `${source === "ai" ? aiProviderName : "Heuristic"} allocation suggestions applied: ` +
            `${assignedToSubsystem} subsystem, ${systemLevelCount} system-level` +
            (newSubsystemCount > 0
                ? `, ${newSubsystemCount} suggest new subsystem`
                : "");
    }

    async function createSubsystemFromHint(reqId: string) {
        if (allocationSuggestLoading || allocationCreateLoading) return;
        if (!canCreateSubsystem()) {
            allocationSuggestError =
                "Current access profile cannot create new subsystems.";
            return;
        }
        const hint = allocationHints.get(reqId);
        const suggestedName = hint?.newSubsystemName?.trim() ?? "";
        if (!suggestedName || !projectId) return;
        allocationSuggestError = "";
        allocationCreateLoading = true;
        try {
            // Check reactive subsystems AND the in-flight creation lock to
            // guard against stale state from rapid clicks.
            const nameKey = suggestedName.toLowerCase();
            const existingNames = new Set(
                subsystems.map((s) => s.name.trim().toLowerCase()),
            );
            const exists =
                existingNames.has(nameKey) ||
                _subsystemCreationLock.has(nameKey);
            if (!exists) {
                _subsystemCreationLock.add(nameKey);
                try {
                    const now = new Date().toISOString();
                    const node: Node = {
                        id: uuidv4(),
                        project_id: projectId,
                        kind: "block",
                        name: suggestedName,
                        description:
                            hint?.rationale ||
                            "Created from requirement allocation suggestion.",
                        data: {
                            kind: "block",
                            is_abstract: false,
                        },
                        meta: {
                            suggested_from_requirement: reqId,
                            actor: hint?.source === "ai" ? "ai" : "system",
                            change_source: hint?.source ?? "heuristic",
                        },
                        created_at: now,
                        modified_at: now,
                    };
                    await saveNode(node);

                    // Auto-link: create a composes edge from the system root
                    // block to the new subsystem block so it appears connected
                    // in BDD diagrams without the user needing to draw it manually.
                    const systemRoot = $nodes.find(
                        (n) =>
                            n.kind === "block" &&
                            !!(n.meta as Record<string, unknown>)?.system_root,
                    );
                    if (systemRoot) {
                        const alreadyLinked = $edges.some(
                            (e) =>
                                e.kind === "composes" &&
                                e.source_id === systemRoot.id &&
                                e.target_id === node.id,
                        );
                        if (!alreadyLinked) {
                            await saveEdge({
                                id: uuidv4(),
                                project_id: projectId,
                                kind: "composes",
                                source_id: systemRoot.id,
                                target_id: node.id,
                                label: "",
                                meta: {},
                                created_at: now,
                                modified_at: now,
                            });
                        }
                    }

                    await loadProject(projectId);
                } finally {
                    _subsystemCreationLock.delete(nameKey);
                }
            }
            setAllocation(reqId, suggestedName);
            setAllocationHint(reqId, null);
            allocationSuggestNotice = exists
                ? `Subsystem "${suggestedName}" already exists; requirement allocation updated.`
                : `Created subsystem "${suggestedName}" and allocated requirement.`;
        } catch (e) {
            allocationSuggestError = `Failed to create subsystem from suggestion: ${String(e)}`;
        } finally {
            allocationCreateLoading = false;
        }
    }

    async function createAllSuggestedSubsystems() {
        if (allocationSuggestLoading || allocationCreateLoading) return;
        if (!canCreateSubsystem()) {
            allocationSuggestError =
                "Current access profile cannot create new subsystems.";
            return;
        }
        if (!projectId) return;
        const hints = _pendingSubsystemHintsReactive;
        if (hints.length === 0) {
            allocationSuggestNotice =
                "No pending subsystem suggestions to create.";
            return;
        }

        allocationSuggestError = "";
        allocationCreateLoading = true;
        try {
            const existing = new Set(
                subsystems.map((sub) => sub.name.trim().toLowerCase()),
            );
            const toCreate = new Map<string, string>();
            for (const hint of hints) {
                const normalized = hint.name.trim();
                const key = normalized.toLowerCase();
                if (!normalized || existing.has(key) || toCreate.has(key)) {
                    continue;
                }
                toCreate.set(key, normalized);
            }

            let createdCount = 0;
            for (const name of toCreate.values()) {
                const nameKey = name.toLowerCase();
                // Skip if another concurrent operation is already creating this
                if (_subsystemCreationLock.has(nameKey)) continue;
                _subsystemCreationLock.add(nameKey);
                try {
                    const now = new Date().toISOString();
                    const relatedHint =
                        hints.find((h) => h.name.toLowerCase() === nameKey) ??
                        null;
                    const node: Node = {
                        id: uuidv4(),
                        project_id: projectId,
                        kind: "block",
                        name,
                        description:
                            relatedHint?.rationale ||
                            "Created from requirement allocation suggestion.",
                        data: {
                            kind: "block",
                            is_abstract: false,
                        },
                        meta: {
                            suggested_from_requirements: true,
                            actor:
                                relatedHint?.source === "ai" ? "ai" : "system",
                            change_source: relatedHint?.source ?? "heuristic",
                        },
                        created_at: now,
                        modified_at: now,
                    };
                    await saveNode(node);

                    // Auto-link to system root block via composes edge
                    const systemRoot = $nodes.find(
                        (n) =>
                            n.kind === "block" &&
                            !!(n.meta as Record<string, unknown>)?.system_root,
                    );
                    if (systemRoot) {
                        const alreadyLinked = $edges.some(
                            (e) =>
                                e.kind === "composes" &&
                                e.source_id === systemRoot.id &&
                                e.target_id === node.id,
                        );
                        if (!alreadyLinked) {
                            await saveEdge({
                                id: uuidv4(),
                                project_id: projectId,
                                kind: "composes",
                                source_id: systemRoot.id,
                                target_id: node.id,
                                label: "",
                                meta: {},
                                created_at: now,
                                modified_at: now,
                            });
                        }
                    }

                    createdCount += 1;
                    existing.add(nameKey);
                } finally {
                    _subsystemCreationLock.delete(nameKey);
                }
            }

            if (createdCount > 0) {
                await loadProject(projectId);
            }

            const hintedReqIds = new Set(hints.map((h) => h.reqId));
            const hintByReqId = new Map(hints.map((h) => [h.reqId, h.name]));
            extractedReqs = extractedReqs.map((req) => {
                const suggestedName = hintByReqId.get(req.id);
                if (!suggestedName || req.imported) return req;
                const allocFlag = `alloc:${suggestedName.toLowerCase().replace(/\s+/g, "_")}`;
                return {
                    ...req,
                    allocation: suggestedName,
                    flags: Array.from(
                        new Set([
                            ...req.flags.filter(
                                (f) =>
                                    !f.startsWith("alloc:") &&
                                    !f.startsWith("suggest:new_subsystem:"),
                            ),
                            allocFlag,
                        ]),
                    ),
                };
            });

            const nextHints = new Map(allocationHints);
            for (const reqId of hintedReqIds) {
                nextHints.delete(reqId);
            }
            allocationHints = nextHints;

            allocationSuggestNotice =
                `Created ${createdCount} subsystem${createdCount === 1 ? "" : "s"} and applied ` +
                `${hintedReqIds.size} allocation${hintedReqIds.size === 1 ? "" : "s"}.`;
        } catch (e) {
            allocationSuggestError = `Failed to create suggested subsystems: ${String(e)}`;
        } finally {
            allocationCreateLoading = false;
        }
    }

    async function suggestRequirementAllocations(
        scope: "all" | "selected" | "single",
        reqId?: string,
    ) {
        if (allocationCreateLoading) return;
        allocationSuggestError = "";
        allocationSuggestNotice = "";
        const targets = allocationTargets(scope, reqId);
        if (targets.length === 0) {
            allocationSuggestNotice =
                scope === "selected"
                    ? "Select at least one requirement to suggest allocations."
                    : "No requirements available for allocation suggestions.";
            return;
        }

        const subsystemInputs = getAllocationSubsystemInputs();
        const fallback = new Map<string, AllocationSuggestionOutput>();
        for (const req of targets) {
            fallback.set(
                req.id,
                heuristicAllocationSuggestion(req, subsystemInputs),
            );
        }

        if (!aiAvailable || aiProviderId === "none") {
            applyAllocationSuggestions(fallback, "heuristic");
            return;
        }

        allocationSuggestLoading = true;
        try {
            const payload: AllocationSuggestionInput[] = targets.map((r) => ({
                id: r.id,
                sentence: r.sentence || r.text,
                name: r.name,
                confidence: r.confidence,
                classification: r.classification ?? "",
                flags: r.flags,
            }));

            const raw = await invoke<string>(
                "ai_suggest_requirement_allocations",
                {
                    requirements: payload,
                    subsystems: subsystemInputs,
                    docType: selectedDoc?.doc_type ?? "General",
                    docName: selectedDoc?.name ?? "document",
                },
            );
            const parsed = JSON.parse(raw) as {
                results?: AllocationSuggestionOutput[];
            };
            const aiResults = parsed.results ?? [];
            const merged = new Map(fallback);
            for (const item of aiResults) {
                const key =
                    item.id?.trim() ||
                    targets.find(
                        (r) =>
                            normalizeKey(r.sentence || r.text) ===
                            normalizeKey(item.sentence || ""),
                    )?.id ||
                    "";
                if (!key) continue;
                merged.set(key, item);
            }
            applyAllocationSuggestions(merged, "ai");
        } catch (e) {
            const msg = String(e);
            allocationSuggestError =
                msg === "no_api_key"
                    ? "No AI provider configured for allocation suggestions. Applied heuristic suggestions instead."
                    : `AI allocation suggestions failed (${msg}). Applied heuristic suggestions instead.`;
            applyAllocationSuggestions(fallback, "heuristic");
        } finally {
            allocationSuggestLoading = false;
        }
    }

    $: allocationCounts = extractedReqs.reduce(
        (acc, r) => {
            const key = r.allocation ?? "System Level";
            acc[key] = (acc[key] ?? 0) + 1;
            return acc;
        },
        {} as Record<string, number>,
    );
    // Compute pending subsystem hint counts inline so Svelte can statically
    // track allocationHints and extractedReqs as reactive dependencies.
    $: _pendingSubsystemHintsReactive = (() => {
        const out: PendingSubsystemHint[] = [];
        for (const req of extractedReqs) {
            if (req.imported) continue;
            const hint = allocationHints.get(req.id);
            const name = hint?.newSubsystemName?.trim() ?? "";
            if (!name) continue;
            out.push({
                reqId: req.id,
                name,
                rationale: hint?.rationale ?? "",
                source: hint?.source ?? "heuristic",
            });
        }
        return out;
    })();
    $: pendingSubsystemHintReqCount = _pendingSubsystemHintsReactive.length;
    $: pendingSubsystemHintUniqueCount = new Set(
        _pendingSubsystemHintsReactive.map((h) => h.name.toLowerCase()),
    ).size;

    $: reviewTextSource = selectedDoc?.text ?? "";
    $: reviewText =
        reviewTextSource.length > MAX_REVIEW_TEXT_LENGTH
            ? reviewTextSource.slice(0, MAX_REVIEW_TEXT_LENGTH)
            : reviewTextSource;
    $: reviewTextTruncated = reviewTextSource.length > MAX_REVIEW_TEXT_LENGTH;
    $: reviewMatches = reviewMode
        ? collectReviewMatches(reviewText, extractedReqs)
        : [];
    $: reviewMatchedRequirementIds = new Set(reviewMatches.map((m) => m.reqId));
    $: reviewUnmatchedCount = reviewMode
        ? extractedReqs.filter((r) => !reviewMatchedRequirementIds.has(r.id))
              .length
        : 0;
    $: {
        reviewMode;
        selectedDoc;
        selectedDocFormat;
        docxPreviewRendered;
        renderedDocHtml;
        renderedPdfUrl;
        extractedReqs;
        void refreshRenderedRequirementHighlights();
    }

    // Export

    type ExportFormat = "csv" | "json" | "txt";

    function exportReqs(format: ExportFormat) {
        const reqs = visibleReqs.filter((r) => !r.imported || true); // export all visible
        const docName = selectedDoc?.name ?? "document";
        const slug = docName
            .replace(/\.[^.]+$/, "")
            .replace(/[^a-z0-9]+/gi, "_")
            .toLowerCase();

        let content = "";
        let mime = "text/plain";
        let filename = "";

        if (format === "csv") {
            const esc = (v: string) => `"${v.replace(/"/g, '""')}"`;
            const header = [
                "ID",
                "Name",
                "Full Text",
                "Confidence",
                "Score",
                "Flags",
                "Source",
            ].join(",");
            const rows = reqs.map((r, i) =>
                [
                    esc(`REQ-${String(i + 1).padStart(3, "0")}`),
                    esc(r.name),
                    esc(r.text),
                    esc(r.confidence),
                    (r.score * 100).toFixed(0) + "%",
                    esc(r.flags.join("; ")),
                    esc(docName),
                ].join(","),
            );
            content = [header, ...rows].join("\r\n");
            mime = "text/csv";
            filename = `${slug}_requirements.csv`;
        } else if (format === "json") {
            const data = reqs.map((r, i) => ({
                id: `REQ-${String(i + 1).padStart(3, "0")}`,
                name: r.name,
                text: r.text,
                confidence: r.confidence,
                score: parseFloat((r.score * 100).toFixed(1)),
                flags: r.flags,
                duplicate: r.duplicate,
                source: docName,
            }));
            content = JSON.stringify(
                { source: docName, requirements: data },
                null,
                2,
            );
            mime = "application/json";
            filename = `${slug}_requirements.json`;
        } else {
            // Plain text - one requirement per block
            content = reqs
                .map(
                    (r, i) =>
                        `REQ-${String(i + 1).padStart(3, "0")}: ${r.name}\n${r.text}\n`,
                )
                .join("\n");
            mime = "text/plain";
            filename = `${slug}_requirements.txt`;
        }

        const blob = new Blob([content], { type: mime });
        const url = URL.createObjectURL(blob);
        const a = document.createElement("a");
        a.href = url;
        a.download = filename;
        a.click();
        URL.revokeObjectURL(url);
    }

    let showExportMenu = false;

    // Document sections

    let sections: DocumentSection[] = [];
    let activeTab: "requirements" | "sections" = "requirements";
    let linkingSection: DocumentSection | null = null; // section being linked to a req
    let requirements = $nodes.filter((n) => n.kind === "requirement") as Node[];
    $: requirements = $nodes.filter((n) => n.kind === "requirement") as Node[];

    // derives edges keyed by section id -> set of req ids
    $: derivesEdges = new Map<string, Set<string>>();

    async function loadSections(docId: string) {
        sections = await invoke<DocumentSection[]>("list_document_sections", {
            documentId: docId,
        });
    }

    async function parseSections(doc: Document) {
        // Parse the document text into sections and persist them
        const lines = doc.text.split(/\r?\n/);
        const parsed: DocumentSection[] = [];
        const now = new Date().toISOString();
        let pos = 0;

        for (const line of lines) {
            const trimmed = line.trim();
            if (!trimmed) continue;

            // Detect section type
            let section_type: DocumentSection["section_type"] = "paragraph";
            let section_ref = "";
            let title = trimmed.slice(0, 120);
            let body = trimmed;

            // Numbered heading: "1.2 Title" or "3.1.2 Title"
            const numHeading = trimmed.match(/^(\d+(?:\.\d+)*)\s+(.+)/);
            if (
                numHeading &&
                numHeading[2].length < 80 &&
                !/\b(shall|must|will)\b/i.test(trimmed)
            ) {
                section_type = "heading";
                section_ref = numHeading[1];
                title = numHeading[2].slice(0, 120);
                body = trimmed;
            }
            // BOM item: "PN-xxxx" or part number pattern
            else if (/^[A-Z]{1,4}-?\d{3,8}\b/.test(trimmed)) {
                section_type = "bom_item";
                const parts = trimmed.split(/\s+/);
                section_ref = parts[0];
                title = parts.slice(1, 8).join(" ");
                body = trimmed;
            }
            // Shall/must/will -> requirement
            else if (
                /\b(shall|must|will)\b/i.test(trimmed) &&
                trimmed.length > 20
            ) {
                section_type = "requirement";
                title = trimmed.slice(0, 120);
                body = trimmed;
            }
            // List item
            else if (
                /^[-\u2022*]\s+/.test(trimmed) ||
                /^\d+\.\s+/.test(trimmed)
            ) {
                section_type = "list_item";
                body = trimmed.replace(/^[-\u2022*\d.]+\s+/, "");
                title = body.slice(0, 80);
            }

            parsed.push({
                id: uuidv4(),
                document_id: doc.id,
                project_id: projectId,
                section_ref,
                section_type,
                title,
                body,
                position: pos++,
                created_at: now,
            });
        }

        // Persist
        await invoke("delete_document_sections", { documentId: doc.id });
        for (const s of parsed) {
            await invoke("upsert_document_section", { section: s });
        }
        sections = parsed;
    }

    function onParseSectionsClick() {
        if (!selectedDoc) return;
        parseSections(selectedDoc);
    }

    async function linkSectionToReq(section: DocumentSection, reqId: string) {
        const now = new Date().toISOString();
        const already = $edges.some(
            (e) =>
                e.kind === "derives" &&
                e.source_id === section.id &&
                e.target_id === reqId,
        );
        if (already) {
            linkingSection = null;
            return;
        }
        await saveEdge({
            id: uuidv4(),
            project_id: projectId,
            kind: "derives",
            source_id: section.id,
            target_id: reqId,
            label: "",
            meta: {},
            created_at: now,
            modified_at: now,
        });
        linkingSection = null;
    }

    const DOC_TYPES = [
        "General",
        "SOW",
        "BOE",
        "BOM",
        "ICD",
        "ConOps",
        "SRS",
        "SDD",
        "Test Plan",
        "Other",
    ];

    async function updateDocType(doc: Document, newType: string) {
        const updated = { ...doc, doc_type: newType };
        await invoke("upsert_document", { doc: updated });
        documents = documents.map((d) => (d.id === doc.id ? updated : d));
    }

    function onDocTypeChange(event: Event) {
        if (!selectedDoc) return;
        const target = event.currentTarget as HTMLSelectElement | null;
        if (!target) return;
        updateDocType(selectedDoc, target.value);
    }

    $: highCount = extractedReqs.filter(
        (r) => r.confidence === "high" && !r.duplicate,
    ).length;
    $: mediumCount = extractedReqs.filter(
        (r) => r.confidence === "medium" && !r.duplicate,
    ).length;
    $: lowCount = extractedReqs.filter(
        (r) => r.confidence === "low" && !r.duplicate,
    ).length;
    $: dupeCount = extractedReqs.filter((r) => r.duplicate).length;
    $: if (
        selectedDoc &&
        extractedReqsDocId === selectedDoc.id &&
        !analyzing &&
        !aiLoading
    ) {
        writeExtractionCache(
            selectedDoc,
            extractedReqs,
            parserMode,
            parserError,
            spacyAvailable,
        );
    }
    $: if (typeof window !== "undefined") {
        window.localStorage.setItem(
            SIDEBAR_WIDTH_STORAGE_KEY,
            String(sidebarWidth),
        );
        window.localStorage.setItem(
            SIDEBAR_COLLAPSED_STORAGE_KEY,
            focusMode
                ? sidebarCollapsedBeforeFocus
                    ? "1"
                    : "0"
                : sidebarCollapsed
                  ? "1"
                  : "0",
        );
        window.localStorage.setItem(
            VIEWER_HEIGHT_STORAGE_KEY,
            String(viewerHeight),
        );
    }

    async function removeDoc(id: string) {
        await invoke("delete_document", { id });
        documents = documents.filter((d) => d.id !== id);
        extractionCache.delete(id);
        if (selectedDocId === id) {
            selectedDocId = documents[0]?.id ?? null;
            if (selectedDoc) runExtraction(selectedDoc);
            else {
                extractedReqs = [];
                extractedReqsDocId = null;
            }
        }
    }

    function fmtSize(bytes: number) {
        if (bytes < 1024) return `${bytes} B`;
        if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
        return `${(bytes / 1024 / 1024).toFixed(1)} MB`;
    }

    function fmtDate(iso: string) {
        return new Date(iso).toLocaleDateString(undefined, {
            month: "short",
            day: "numeric",
            year: "numeric",
        });
    }

    function setAIProvider(providerName: string) {
        aiProviderId = providerName;
        aiProviderName =
            providerName === "ollama"
                ? "Ollama"
                : providerName === "anthropic"
                  ? "Claude"
                  : providerName;
        if (providerName !== "ollama") {
            useGraphRag = false;
        }
    }

    function loadPanePreferences() {
        if (typeof window === "undefined") return;
        const storedSidebar = Number(
            window.localStorage.getItem(SIDEBAR_WIDTH_STORAGE_KEY),
        );
        if (Number.isFinite(storedSidebar)) {
            sidebarWidth = clamp(
                storedSidebar,
                MIN_SIDEBAR_WIDTH,
                MAX_SIDEBAR_WIDTH,
            );
        }

        const storedViewer = Number(
            window.localStorage.getItem(VIEWER_HEIGHT_STORAGE_KEY),
        );
        if (Number.isFinite(storedViewer)) {
            viewerHeight = clamp(storedViewer, MIN_VIEWER_HEIGHT, 900);
        } else {
            viewerHeight = DEFAULT_VIEWER_HEIGHT;
        }

        const storedCollapsed = window.localStorage.getItem(
            SIDEBAR_COLLAPSED_STORAGE_KEY,
        );
        sidebarCollapsed = storedCollapsed === "1";
    }

    // Lifecycle

    onMount(async () => {
        loadPanePreferences();
        if (
            typeof navigator !== "undefined" &&
            /Mac|iPhone|iPad|iPod/i.test(navigator.platform)
        ) {
            docSwitcherShortcutLabel = "Cmd+K";
        }
        await loadProject(projectId);
        await refreshDocuments();
        try {
            aiAvailable = await invoke<boolean>("ai_available");
            if (aiAvailable) {
                const name = await invoke<string>("ai_provider_name").catch(
                    () => "none",
                );
                setAIProvider(name);
            } else {
                setAIProvider("none");
            }
        } catch {
            aiAvailable = false;
            setAIProvider("none");
        }
        if (documents.length > 0) {
            selectedDocId = documents[0].id;
            runExtraction(documents[0]);
            await loadSections(documents[0].id);
        }
    });

    onDestroy(() => {
        stopResizing();
        clearRenderedViewerContent();
    });

    // Re-run extraction + load sections when selected doc changes
    $: if (selectedDoc) {
        clearReviewSelection();
        renderDocumentWithFormatting(selectedDoc);
        const hydrated = hydrateReqListFromCache(selectedDoc);
        if (!hydrated && extractedReqsDocId !== selectedDoc.id) {
            extractedReqs = [];
            extractedReqsDocId = null;
            parserError = "";
        }
        runExtraction(selectedDoc);
        loadSections(selectedDoc.id);
        // Re-check AI availability in case the key was just configured
        invoke<boolean>("ai_available")
            .then((v) => {
                aiAvailable = v;
                if (v) {
                    invoke<string>("ai_provider_name")
                        .then((n) => setAIProvider(n))
                        .catch(() => {});
                } else {
                    setAIProvider("none");
                }
            })
            .catch(() => {});
    } else {
        clearRenderedViewerContent();
        extractedReqsDocId = null;
        aiError = "";
        aiNotice = "";
        permissionNotice = "";
        clearAllocationSuggestionState();
    }

    $: if (
        selectedDoc &&
        selectedDocFormat === "docx" &&
        renderedDocxBuffer &&
        docxRenderHost
    ) {
        const attemptKey = `${selectedDoc.id}:${viewerRenderToken}`;
        if (docxRenderAttemptKey !== attemptKey) {
            docxRenderAttemptKey = attemptKey;
            const currentToken = viewerRenderToken;
            void tryRenderDocxPreview(renderedDocxBuffer, currentToken).then(
                (ok) => {
                    if (currentToken !== viewerRenderToken) return;
                    if (ok) {
                        renderedDocHtml = "";
                        viewerRenderError = "";
                    } else if (lastDocxPreviewError) {
                        viewerRenderError = `Using fallback DOCX renderer. docx-preview failed: ${lastDocxPreviewError}`;
                    } else if (docxPreviewAttempted) {
                        viewerRenderError =
                            "Using fallback DOCX renderer. Word-like page preview is unavailable in this session.";
                    }
                },
            );
        }
    }
</script>

<!-- ----------------------------------------------------------------------- -->

<svelte:window
    on:click={() => (showExportMenu = false)}
    on:keydown={handleGlobalKeydown}
    on:mousemove={onGlobalMouseMove}
    on:mouseup={stopResizing}
    on:blur={stopResizing}
/>

<div
    class="docs-root page-frame"
    class:focus-mode={focusMode}
    class:is-resizing={resizingSidebar || resizingViewer}
>
    <!-- Header -->
    <header class="docs-header page-header">
        <div>
            <div class="page-eyebrow">Documents</div>
            <h1 class="page-title">Document Library & Requirement Extractor</h1>
            <p class="page-subtitle">
                Upload .docx, .pdf, or .txt files - shall/must/will statements
                are automatically extracted as requirements.
            </p>
        </div>
    </header>

    <div
        class="docs-body page-body"
        class:sidebar-collapsed={sidebarCollapsed || focusMode}
        bind:this={docsBodyEl}
        style={`--sidebar-width: ${effectiveSidebarWidth}px;`}
    >
        <!-- Left: document list -->
        <aside
            class="doc-sidebar"
            class:collapsed={sidebarCollapsed || focusMode}
        >
            <div class="sidebar-header">
                <span class="sidebar-title">Documents</span>
                <div class="sidebar-actions">
                    <button
                        class="btn-icon"
                        on:click={toggleSidebarCollapsed}
                        title={sidebarCollapsed || focusMode
                            ? "Show document list"
                            : "Hide document list"}
                    >
                        {sidebarCollapsed || focusMode ? ">" : "<"}
                    </button>
                    <button
                        class="btn-icon"
                        on:click={() => fileInput.click()}
                        title="Upload document">+</button
                    >
                </div>
            </div>

            <!-- Drop zone -->
            <div
                class="drop-zone"
                class:drag-over={dragOver}
                role="button"
                tabindex="0"
                aria-label="Drop files here"
                on:dragover|preventDefault={() => (dragOver = true)}
                on:dragleave={() => (dragOver = false)}
                on:drop|preventDefault={(e) => {
                    dragOver = false;
                    handleFiles(e.dataTransfer?.files ?? null);
                }}
                on:click={() => fileInput.click()}
                on:keydown={(e) => e.key === "Enter" && fileInput.click()}
            >
                {#if parsing}
                    <span class="drop-hint">Parsing...</span>
                {:else}
                    <span class="drop-hint">Drop files or click to upload</span>
                    <span class="drop-sub">.docx, .pdf, .txt, .md</span>
                {/if}
            </div>

            <input
                bind:this={fileInput}
                type="file"
                accept=".docx,.pdf,.txt,.md"
                multiple
                style="display:none"
                on:change={(e) => handleFiles(e.currentTarget.files)}
            />

            <!-- Document list -->
            <div class="doc-list">
                {#if documents.length === 0}
                    <div class="doc-empty">
                        <Upload size={18} />
                        <span>No documents yet</span>
                    </div>
                {:else}
                    {#each documents as doc (doc.id)}
                        {@const docFormat = detectFileFormat(doc)}
                        <button
                            class="doc-item"
                            class:active={selectedDocId === doc.id}
                            on:click={() => (selectedDocId = doc.id)}
                        >
                            <span class="doc-icon">
                                {#if docFormat === "pdf"}<FileText
                                        size={14}
                                    />{:else if docFormat === "docx"}<BookOpen
                                        size={14}
                                    />{:else}<FileIcon size={14} />{/if}
                            </span>
                            <div class="doc-item-info">
                                <span class="doc-item-name">{doc.name}</span>
                                <span class="doc-item-meta"
                                    >{fmtSize(doc.size)} - {fmtDate(
                                        doc.added_at,
                                    )}</span
                                >
                            </div>
                            <button
                                class="doc-remove"
                                on:click|stopPropagation={() =>
                                    removeDoc(doc.id)}
                                title="Remove">x</button
                            >
                        </button>
                    {/each}
                {/if}
            </div>
        </aside>

        <div
            class="pane-resizer pane-resizer-vertical sidebar-resizer"
            class:hidden={sidebarCollapsed || focusMode}
            role="separator"
            aria-orientation="vertical"
            aria-label="Resize document sidebar"
            title="Drag to resize sidebar"
            on:mousedown={startSidebarResize}
        ></div>

        <!-- Right: extractor -->
        <div class="extractor">
            {#if !selectedDoc}
                <div class="extractor-empty">
                    <Inbox size={40} class="extractor-empty-icon" />
                    <div class="extractor-empty-title">
                        No document selected
                    </div>
                    <div class="extractor-empty-body">
                        Upload a .docx, .pdf, or .txt file to automatically
                        extract requirements.
                    </div>
                </div>
            {:else}
                <!-- Doc info bar -->
                <div class="extractor-header">
                    <div class="extractor-doc-info">
                        <span class="extractor-doc-name"
                            >{selectedDoc.name}</span
                        >
                        <div class="extractor-doc-meta-row">
                            <span class="extractor-doc-meta"
                                >{fmtSize(selectedDoc.size)} - Added {fmtDate(
                                    selectedDoc.added_at,
                                )}</span
                            >
                            <select
                                class="doc-type-select"
                                value={selectedDoc.doc_type.toUpperCase()}
                                on:change={onDocTypeChange}
                            >
                                {#each DOC_TYPES as t}
                                    <option value={t}>{t}</option>
                                {/each}
                            </select>
                        </div>
                    </div>
                    <div class="extractor-actions">
                        {#if sidebarCollapsed || focusMode}
                            <button
                                class="btn-ghost-sm"
                                on:click={() => fileInput.click()}
                                title="Upload document"
                            >
                                Upload
                            </button>
                        {/if}
                        {#if !focusMode}
                            <button
                                class="btn-ghost-sm"
                                on:click={toggleSidebarCollapsed}
                            >
                                {sidebarCollapsed ? "Show docs" : "Hide docs"}
                            </button>
                        {/if}
                        <button
                            class="btn-ghost-sm"
                            class:focus-active={focusMode}
                            on:click={toggleFocusMode}
                            title="Focus on document and requirements workspace"
                        >
                            {focusMode ? "Exit focus" : "Focus mode"}
                        </button>
                        <button
                            class="btn-ghost-sm"
                            on:click={() => void openDocSwitcher()}
                            title={`Quick document switcher (${docSwitcherShortcutLabel})`}
                        >
                            Switch ({docSwitcherShortcutLabel})
                        </button>
                        {#if extractedReqs.length > 0}
                            <button class="btn-ghost-sm" on:click={selectAll}
                                >Select all</button
                            >
                            <button class="btn-ghost-sm" on:click={selectNone}
                                >None</button
                            >
                        {/if}
                        <button class="btn-ghost-sm" on:click={toggleReviewMode}
                            >{reviewMode
                                ? "Hide highlights"
                                : "Review highlights"}</button
                        >
                        <div
                            class="view-toggle-group"
                            role="group"
                            aria-label="Switch document workspace view"
                        >
                            <button
                                class="view-toggle-btn"
                                class:active={workspaceView === "viewer"}
                                on:click={() => (workspaceView = "viewer")}
                            >
                                Document
                            </button>
                            <button
                                class="view-toggle-btn"
                                class:active={workspaceView === "requirements"}
                                on:click={() =>
                                    (workspaceView = "requirements")}
                            >
                                Requirements
                            </button>
                            <button
                                class="view-toggle-btn"
                                class:active={workspaceView === "split"}
                                on:click={() => (workspaceView = "split")}
                            >
                                Split
                            </button>
                        </div>
                        {#if aiAvailable && isOllamaProvider && extractedReqs.length === 0}
                            <label
                                class="graphrag-toggle"
                                class:active={useGraphRag}
                                title="Use GraphRAG entity and relationship context to enrich extraction prompts"
                            >
                                <input
                                    type="checkbox"
                                    bind:checked={useGraphRag}
                                />
                                <span>Use GraphRAG</span>
                            </label>
                        {/if}
                        <button
                            class="btn-ai"
                            on:click={() =>
                                selectedDoc && runAIExtraction(selectedDoc)}
                            disabled={aiLoading || !selectedDoc}
                            title={aiAvailable
                                ? extractedReqs.length > 0
                                    ? `Run AI quality review on extracted requirements with ${aiProviderName}`
                                    : `Extract requirements with ${aiRunLabel} (replaces current results)`
                                : "Configure an AI provider in Integrations to enable AI extraction"}
                            class:unavailable={!aiAvailable}
                        >
                            {#if aiLoading}
                                <span class="btn-spinner">...</span> Analyzing...
                            {:else}
                                {extractedReqs.length > 0
                                    ? "Review with AI"
                                    : isOllamaProvider && useGraphRag
                                      ? "Analyze with GraphRAG"
                                      : "Analyze with AI"}
                            {/if}
                        </button>
                        {#if extractedReqs.length > 0}
                            <button
                                class="btn-ghost-sm"
                                on:click={() =>
                                    suggestRequirementAllocations("selected")}
                                disabled={allocationSuggestLoading ||
                                    allocationCreateLoading ||
                                    extractedReqs.filter(
                                        (r) => r.selected && !r.imported,
                                    ).length === 0}
                                title="Use AI to suggest subsystem/system-level allocations for selected requirements"
                            >
                                {allocationSuggestLoading
                                    ? "Suggesting..."
                                    : "Suggest allocations"}
                            </button>
                            <button
                                class="btn-ghost-sm"
                                on:click={createAllSuggestedSubsystems}
                                disabled={allocationCreateLoading ||
                                    allocationSuggestLoading ||
                                    pendingSubsystemHintReqCount === 0 ||
                                    !canCreateSubsystem()}
                                title="Create all suggested subsystems and apply allocations"
                            >
                                {allocationCreateLoading
                                    ? "Creating..."
                                    : `Create suggested (${pendingSubsystemHintUniqueCount})`}
                            </button>
                            <!-- Export dropdown -->
                            <div class="export-wrap">
                                <button
                                    class="btn-ghost-sm"
                                    on:click|stopPropagation={() =>
                                        (showExportMenu = !showExportMenu)}
                                    title="Export extracted requirements"
                                >
                                    Export v
                                </button>
                                {#if showExportMenu}
                                    <div
                                        class="export-menu"
                                        role="menu"
                                        on:click|stopPropagation
                                    >
                                        <button
                                            class="export-item"
                                            on:click={() => {
                                                exportReqs("csv");
                                                showExportMenu = false;
                                            }}
                                        >
                                            <span class="export-icon">v</span>
                                            <span>
                                                <strong>CSV</strong>
                                                <span class="export-desc"
                                                    >Spreadsheet (Excel, Sheets)</span
                                                >
                                            </span>
                                        </button>
                                        <button
                                            class="export-item"
                                            on:click={() => {
                                                exportReqs("json");
                                                showExportMenu = false;
                                            }}
                                        >
                                            <span class="export-icon">v</span>
                                            <span>
                                                <strong>JSON</strong>
                                                <span class="export-desc"
                                                    >Structured data / import</span
                                                >
                                            </span>
                                        </button>
                                        <button
                                            class="export-item"
                                            on:click={() => {
                                                exportReqs("txt");
                                                showExportMenu = false;
                                            }}
                                        >
                                            <span class="export-icon">v</span>
                                            <span>
                                                <strong>Plain text</strong>
                                                <span class="export-desc"
                                                    >One requirement per block</span
                                                >
                                            </span>
                                        </button>
                                    </div>
                                {/if}
                            </div>
                            <button
                                class="btn-primary-sm"
                                disabled={selectedImportableCount === 0 ||
                                    importing}
                                on:click={importSelected}
                            >
                                {importing
                                    ? "Importing..."
                                    : `Import ${selectedImportableCount} selected`}
                            </button>
                        {/if}
                    </div>
                </div>

                <!-- Tab bar -->
                <div class="doc-tab-bar">
                    <button
                        class="doc-tab"
                        class:active={activeTab === "requirements"}
                        on:click={() => (activeTab = "requirements")}
                    >
                        <List size={13} />
                        Requirements
                        {#if extractedReqs.length > 0}
                            <span class="tab-count">{extractedReqs.length}</span
                            >
                        {/if}
                    </button>
                    <button
                        class="doc-tab"
                        class:active={activeTab === "sections"}
                        on:click={() => (activeTab = "sections")}
                    >
                        <Layers size={13} />
                        Sections
                        {#if sections.length > 0}
                            <span class="tab-count">{sections.length}</span>
                        {/if}
                    </button>
                </div>

                {#if activeTab === "sections"}
                    <!-- Sections panel -->
                    <div class="sections-panel">
                        <div class="sections-toolbar">
                            <span class="sections-hint">
                                Parse document into structured sections, then
                                link sections to requirements.
                            </span>
                            <button
                                class="btn-ghost-sm"
                                on:click={onParseSectionsClick}
                            >
                                <Layers size={12} />
                                Parse sections
                            </button>
                        </div>

                        {#if sections.length === 0}
                            <div class="sections-empty">
                                <Layers size={32} />
                                <span>No sections parsed yet.</span>
                                <span class="sections-empty-sub"
                                    >Click "Parse sections" to extract structure
                                    from this document.</span
                                >
                            </div>
                        {:else}
                            <div class="section-list">
                                {#each sections as sec (sec.id)}
                                    {@const linkedReqIds = new Set(
                                        $edges
                                            .filter(
                                                (e) =>
                                                    e.kind === "derives" &&
                                                    e.source_id === sec.id,
                                            )
                                            .map((e) => e.target_id),
                                    )}
                                    <div
                                        class="section-card type-{sec.section_type}"
                                    >
                                        <div class="section-card-top">
                                            <span class="section-type-badge"
                                                >{sec.section_type.replace(
                                                    "_",
                                                    " ",
                                                )}</span
                                            >
                                            {#if sec.section_ref}
                                                <span class="section-ref"
                                                    >{sec.section_ref}</span
                                                >
                                            {/if}
                                            <span class="section-title"
                                                >{sec.title}</span
                                            >
                                            <div class="section-actions">
                                                {#if linkedReqIds.size > 0}
                                                    <span class="linked-badge">
                                                        <Link size={10} />
                                                        {linkedReqIds.size}
                                                    </span>
                                                {/if}
                                                <button
                                                    class="btn-link-req"
                                                    on:click={() =>
                                                        (linkingSection =
                                                            linkingSection?.id ===
                                                            sec.id
                                                                ? null
                                                                : sec)}
                                                    title="Link to requirement"
                                                >
                                                    <Link size={12} />
                                                    Link
                                                </button>
                                            </div>
                                        </div>
                                        {#if sec.body && sec.body !== sec.title}
                                            <p class="section-body">
                                                {sec.body.slice(0, 200)}{sec
                                                    .body.length > 200
                                                    ? "..."
                                                    : ""}
                                            </p>
                                        {/if}
                                        {#if linkingSection?.id === sec.id}
                                            <div class="link-req-picker">
                                                <span class="picker-label"
                                                    >Link to requirement:</span
                                                >
                                                {#if requirements.length === 0}
                                                    <span class="picker-empty"
                                                        >No requirements yet -
                                                        import some first.</span
                                                    >
                                                {:else}
                                                    <select
                                                        class="picker-select"
                                                        on:change={(e) => {
                                                            if (
                                                                e.currentTarget
                                                                    .value
                                                            ) {
                                                                linkSectionToReq(
                                                                    sec,
                                                                    e
                                                                        .currentTarget
                                                                        .value,
                                                                );
                                                            }
                                                        }}
                                                    >
                                                        <option value=""
                                                            >-- pick a
                                                            requirement --</option
                                                        >
                                                        {#each requirements as req (req.id)}
                                                            <option
                                                                value={req.id}
                                                                disabled={linkedReqIds.has(
                                                                    req.id,
                                                                )}
                                                            >
                                                                {req.data
                                                                    ?.req_id ??
                                                                    req.id} - {req.name}
                                                                {linkedReqIds.has(
                                                                    req.id,
                                                                )
                                                                    ? " (linked)"
                                                                    : ""}
                                                            </option>
                                                        {/each}
                                                    </select>
                                                {/if}
                                                <button
                                                    class="btn-ghost-sm"
                                                    on:click={() =>
                                                        (linkingSection = null)}
                                                    >Cancel</button
                                                >
                                            </div>
                                        {/if}
                                    </div>
                                {/each}
                            </div>
                        {/if}
                    </div>
                {:else}
                    <div class="requirements-workspace">
                        <!-- Status banners -->
                        {#if aiLoading}
                            <div class="sidecar-banner analyzing">
                                <span class="banner-spinner" aria-hidden="true"
                                ></span>
                                {extractedReqs.length > 0
                                    ? `${aiProviderName} is reviewing extracted requirements`
                                    : `${aiRunLabel} is reading the document - this may take a moment...`}
                            </div>
                        {:else if analyzing}
                            <div class="sidecar-banner analyzing">
                                <span class="banner-spinner" aria-hidden="true"
                                ></span> Extracting requirements...
                            </div>
                        {/if}
                        {#if parserError && !aiLoading}
                            <div
                                class="sidecar-banner warning"
                                transition:slide={{ duration: 180, axis: "y" }}
                            >
                                Python extractor failed - fell back to
                                heuristics. {parserError}
                            </div>
                        {/if}
                        {#if aiError}
                            <div
                                class="sidecar-banner warning"
                                transition:slide={{ duration: 180, axis: "y" }}
                            >
                                AI extraction failed - {aiError}
                            </div>
                        {/if}
                        {#if aiNotice && !aiError && !aiLoading}
                            <div
                                class="sidecar-banner ok"
                                transition:slide={{ duration: 180, axis: "y" }}
                            >
                                {aiNotice}
                            </div>
                        {/if}
                        {#if permissionNotice}
                            <div
                                class="sidecar-banner warning"
                                transition:slide={{ duration: 180, axis: "y" }}
                            >
                                {permissionNotice}
                            </div>
                        {/if}
                        {#if allocationSuggestLoading}
                            <div class="sidecar-banner analyzing">
                                <span class="banner-spinner" aria-hidden="true"
                                ></span>
                                Suggesting requirement allocations...
                            </div>
                        {/if}
                        {#if allocationCreateLoading}
                            <div class="sidecar-banner analyzing">
                                <span class="banner-spinner" aria-hidden="true"
                                ></span>
                                Creating suggested subsystems...
                            </div>
                        {/if}
                        {#if allocationSuggestError && !allocationSuggestLoading && !allocationCreateLoading}
                            <div class="sidecar-banner warning">
                                {allocationSuggestError}
                            </div>
                        {/if}
                        {#if allocationSuggestNotice && !allocationSuggestError && !allocationSuggestLoading}
                            <div class="sidecar-banner ok">
                                {allocationSuggestNotice}
                            </div>
                        {/if}

                        <div
                            class="workspace-panels"
                            class:split={workspaceView === "split"}
                        >
                            {#if showDocumentPanel}
                                <div
                                    class="document-center"
                                    class:full-height={workspaceView ===
                                        "viewer" || workspaceView === "split"}
                                >
                                    <div class="document-center-head">
                                        <div class="document-center-title-wrap">
                                            <div class="document-center-title">
                                                Document Viewer
                                            </div>
                                            <div class="document-center-file">
                                                {selectedDoc.name}
                                            </div>
                                        </div>
                                        <div class="document-center-meta">
                                            <span class="document-meta-pill"
                                                >{selectedDocFormat.toUpperCase()}</span
                                            >
                                            <span class="document-meta-pill"
                                                >{selectedDoc.text.length.toLocaleString()}
                                                chars</span
                                            >
                                            {#if reviewMode}
                                                <span
                                                    class="document-meta-pill active"
                                                    >{reviewMatches.length} highlighted
                                                    span{reviewMatches.length !==
                                                    1
                                                        ? "s"
                                                        : ""}</span
                                                >
                                                {#if reviewUnmatchedCount > 0}
                                                    <span
                                                        class="document-meta-pill warning"
                                                        >{reviewUnmatchedCount} requirement{reviewUnmatchedCount !==
                                                        1
                                                            ? "s"
                                                            : ""} unmatched</span
                                                    >
                                                {/if}
                                            {/if}
                                        </div>
                                    </div>
                                    <div class="document-center-help">
                                        {#if reviewMode}
                                            {#if selectedDocFormat === "pdf"}
                                                Highlight review is active.
                                                Embedded PDF view is preserved;
                                                use requirement list actions to
                                                adjust extracted items.
                                            {:else}
                                                Highlight review is active.
                                                Formatted document view stays
                                                enabled; use selection +
                                                requirement list actions to
                                                adjust extracted items.
                                            {/if}
                                        {:else if selectedDocFormat === "pdf"}
                                            Native PDF preview enabled when
                                            source payload is available. Toggle <strong
                                                >Review highlights</strong
                                            > to edit extracted requirements.
                                        {:else if selectedDocFormat === "docx"}
                                            Formatted DOCX preview enabled when
                                            source payload is available. Toggle <strong
                                                >Review highlights</strong
                                            > to edit extracted requirements.
                                        {:else}
                                            Full extracted document text
                                            preview.
                                        {/if}
                                    </div>
                                    <div
                                        class="document-center-viewer"
                                        bind:this={reviewViewer}
                                        style={workspaceView === "split"
                                            ? "height: 100%; min-height: 0;"
                                            : "height: 100%; min-height: 320px;"}
                                        tabindex="0"
                                        role="textbox"
                                        aria-label="Document text viewer"
                                        on:mouseup={captureReviewSelection}
                                        on:keyup={captureReviewSelection}
                                        on:dblclick={handleViewerDoubleClick}
                                    >
                                        {#if viewerRenderLoading}
                                            <div
                                                class="document-center-empty viewer-loading"
                                                in:fade={{ duration: 150 }}
                                            >
                                                <div
                                                    class="spinner spinner-lg"
                                                    aria-hidden="true"
                                                ></div>
                                                <span
                                                    >Rendering formatted
                                                    preview�</span
                                                >
                                            </div>
                                        {:else if selectedDocFormat === "pdf" && renderedPdfUrl}
                                            <div class="pdf-stage">
                                                <object
                                                    class="document-pdf-frame"
                                                    data={renderedPdfUrl}
                                                    type={selectedDoc.source_mime ||
                                                        "application/pdf"}
                                                >
                                                    <span
                                                        class="document-center-empty"
                                                        >PDF preview is
                                                        unavailable in this
                                                        environment.</span
                                                    >
                                                </object>
                                            </div>
                                        {:else if selectedDocFormat === "docx" && renderedDocHtml}
                                            <div
                                                class="docx-preview-host"
                                                class:active={docxPreviewRendered}
                                                bind:this={docxRenderHost}
                                            ></div>
                                            {#if !docxPreviewRendered}
                                                {#if viewerRenderError}
                                                    <div
                                                        class="document-render-error"
                                                    >
                                                        {viewerRenderError}
                                                    </div>
                                                {/if}
                                                <div
                                                    class="docx-rendered"
                                                    bind:this={docxHtmlHost}
                                                >
                                                    {@html renderedDocHtml}
                                                </div>
                                            {/if}
                                        {:else if selectedDocFormat === "docx"}
                                            <div
                                                class="docx-preview-host"
                                                class:active={docxPreviewRendered}
                                                bind:this={docxRenderHost}
                                            ></div>
                                            {#if viewerRenderError}
                                                <div
                                                    class="document-render-error"
                                                >
                                                    {viewerRenderError}
                                                </div>
                                            {/if}
                                            <pre
                                                class="document-full-text"
                                                bind:this={plainTextHost}>
                                    >{selectedDoc.text}</pre>
                                        {:else}
                                            {#if viewerRenderError}
                                                <div
                                                    class="document-render-error"
                                                >
                                                    {viewerRenderError}
                                                </div>
                                            {/if}
                                            <pre
                                                class="document-full-text"
                                                bind:this={plainTextHost}>
                                    >{selectedDoc.text}</pre>
                                        {/if}
                                    </div>
                                    {#if reviewMode && reviewSelection}
                                        <div class="highlight-review-selection">
                                            <span class="selection-label"
                                                >Selected ({reviewSelection.length}
                                                chars):</span
                                            >
                                            <span class="selection-preview"
                                                >{reviewSelection.slice(
                                                    0,
                                                    240,
                                                )}{reviewSelection.length > 240
                                                    ? "..."
                                                    : ""}</span
                                            >
                                            <button
                                                class="btn-primary-sm"
                                                on:click={addRequirementFromSelection}
                                                disabled={reviewSelection.length <
                                                    MIN_MANUAL_HIGHLIGHT_LEN}
                                            >
                                                Add selection
                                            </button>
                                            <button
                                                class="btn-ghost-sm"
                                                on:click={clearReviewSelection}
                                            >
                                                Clear
                                            </button>
                                        </div>
                                    {/if}
                                </div>
                            {/if}

                            <!-- Extracted requirements -->
                            {#if showRequirementsPanel}
                                <div
                                    class="requirements-panel"
                                    class:full-height={!showDocumentPanel ||
                                        workspaceView === "split"}
                                    class:split-compact={workspaceView ===
                                        "split"}
                                >
                                    {#if extractedReqs.length === 0 && !analyzing}
                                        <div class="no-reqs">
                                            No shall/must/will statements found
                                            in this document.
                                        </div>
                                    {:else}
                                        <!-- Confidence filter toggles -->

                                        <div class="confidence-filters">
                                            <span class="filter-label"
                                                >Show:</span
                                            >
                                            <button
                                                class="conf-toggle high"
                                                class:active={showHigh}
                                                on:click={() =>
                                                    (showHigh = !showHigh)}
                                                >High {highCount}</button
                                            >
                                            <button
                                                class="conf-toggle medium"
                                                class:active={showMedium}
                                                on:click={() =>
                                                    (showMedium = !showMedium)}
                                                >Medium {mediumCount}</button
                                            >
                                            <button
                                                class="conf-toggle low"
                                                class:active={showLow}
                                                on:click={() =>
                                                    (showLow = !showLow)}
                                                >Low {lowCount}</button
                                            >
                                            {#if dupeCount > 0}
                                                <button
                                                    class="conf-toggle dupe"
                                                    class:active={showDupes}
                                                    on:click={() =>
                                                        (showDupes =
                                                            !showDupes)}
                                                    >Dupes {dupeCount}</button
                                                >
                                            {/if}
                                        </div>

                                        <div class="allocation-bar">
                                            <span class="filter-label"
                                                >Allocation:</span
                                            >
                                            <span class="alloc-pill"
                                                >System Level {allocationCounts[
                                                    "System Level"
                                                ] ?? 0}</span
                                            >
                                            {#each subsystems as s (s.id)}
                                                <span class="alloc-pill"
                                                    >{s.name}
                                                    {allocationCounts[s.name] ??
                                                        0}</span
                                                >
                                            {/each}
                                            <div class="allocation-actions">
                                                <button
                                                    class="btn-ghost-sm"
                                                    on:click={() =>
                                                        suggestRequirementAllocations(
                                                            "selected",
                                                        )}
                                                    disabled={allocationSuggestLoading ||
                                                        allocationCreateLoading ||
                                                        extractedReqs.filter(
                                                            (r) =>
                                                                r.selected &&
                                                                !r.imported,
                                                        ).length === 0}
                                                >
                                                    Suggest selected
                                                </button>
                                                <button
                                                    class="btn-ghost-sm"
                                                    on:click={() =>
                                                        suggestRequirementAllocations(
                                                            "all",
                                                        )}
                                                    disabled={allocationSuggestLoading ||
                                                        allocationCreateLoading ||
                                                        extractedReqs.length ===
                                                            0}
                                                >
                                                    Suggest all
                                                </button>
                                                {#if pendingSubsystemHintReqCount > 0}
                                                    <button
                                                        class="btn-ghost-sm"
                                                        on:click={createAllSuggestedSubsystems}
                                                        disabled={allocationCreateLoading ||
                                                            allocationSuggestLoading}
                                                        title="Create all suggested subsystems and apply allocations"
                                                    >
                                                        Create suggested ({pendingSubsystemHintUniqueCount}
                                                        subsystem{pendingSubsystemHintUniqueCount !==
                                                        1
                                                            ? "s"
                                                            : ""})
                                                    </button>
                                                {/if}
                                            </div>
                                        </div>

                                        <div class="req-count-bar">
                                            <span
                                                >{visibleReqs.length} of {extractedReqs.length}
                                                statement{extractedReqs.length !==
                                                1
                                                    ? "s"
                                                    : ""} shown</span
                                            >
                                            <span class="req-count-imported"
                                                >{extractedReqs.filter(
                                                    (r) => r.imported,
                                                ).length} already imported</span
                                            >
                                        </div>

                                        <div class="req-extract-list">
                                            {#each visibleReqs as req (req.id)}
                                                {@const allocHint =
                                                    allocationHints.get(req.id)}
                                                <div
                                                    class="req-extract-card conf-{req.confidence}"
                                                    class:imported={req.imported}
                                                    class:selected={req.selected}
                                                    class:is-dupe={req.duplicate}
                                                >
                                                    <div
                                                        class="req-extract-top"
                                                    >
                                                        <label
                                                            class="req-extract-check"
                                                        >
                                                            <input
                                                                type="checkbox"
                                                                checked={req.selected ??
                                                                    false}
                                                                disabled={req.imported}
                                                                on:change={() =>
                                                                    toggleSelect(
                                                                        req.id,
                                                                    )}
                                                            />
                                                        </label>
                                                        <div
                                                            class="req-extract-body"
                                                        >
                                                            <div
                                                                class="req-extract-name"
                                                            >
                                                                {req.name}
                                                                {#if req.duplicate}<span
                                                                        class="dupe-badge"
                                                                        >duplicate</span
                                                                    >{/if}
                                                            </div>
                                                            <p
                                                                class="req-extract-text"
                                                                title={req.text}
                                                            >
                                                                {req.text}
                                                            </p>
                                                            {#if req.flags.length > 0 || req.classification}
                                                                <div
                                                                    class="req-flags"
                                                                >
                                                                    {#if req.classification}
                                                                        <span
                                                                            class="flag-pill"
                                                                            >class:{req.classification}</span
                                                                        >
                                                                    {/if}
                                                                    {#each req.flags as flag}
                                                                        <span
                                                                            class="flag-pill"
                                                                            >{flag}</span
                                                                        >
                                                                    {/each}
                                                                    <span
                                                                        class="score-pill"
                                                                        >score: {(
                                                                            req.score *
                                                                            100
                                                                        ).toFixed(
                                                                            0,
                                                                        )}%</span
                                                                    >
                                                                </div>
                                                            {/if}
                                                        </div>
                                                        <div
                                                            class="req-extract-actions"
                                                        >
                                                            {#if req.imported}
                                                                <span
                                                                    class="imported-badge"
                                                                    >Imported</span
                                                                >
                                                            {:else}
                                                                <select
                                                                    class="alloc-select"
                                                                    value={req.allocation ??
                                                                        ""}
                                                                    on:change={(
                                                                        e,
                                                                    ) =>
                                                                        setAllocation(
                                                                            req.id,
                                                                            e
                                                                                .currentTarget
                                                                                .value,
                                                                        )}
                                                                    title="Allocate to subsystem"
                                                                >
                                                                    <option
                                                                        value=""
                                                                        >System
                                                                        Level</option
                                                                    >
                                                                    {#each subsystems as sub (sub.id)}
                                                                        <option
                                                                            value={sub.name}
                                                                            >{sub.name}</option
                                                                        >
                                                                    {/each}
                                                                </select>
                                                                <button
                                                                    class="btn-ghost-sm"
                                                                    on:click={() =>
                                                                        suggestRequirementAllocations(
                                                                            "single",
                                                                            req.id,
                                                                        )}
                                                                    disabled={allocationSuggestLoading ||
                                                                        allocationCreateLoading}
                                                                    title="Suggest best allocation for this requirement"
                                                                >
                                                                    Suggest
                                                                </button>
                                                                <span
                                                                    class="conf-badge conf-badge-{req.confidence}"
                                                                    >{req.confidence}</span
                                                                >
                                                                <button
                                                                    class="btn-import-one"
                                                                    on:click={() =>
                                                                        importOne(
                                                                            req,
                                                                        )}
                                                                    disabled={importing ||
                                                                        !canImportRequirement(
                                                                            req,
                                                                        )}
                                                                    >Import</button
                                                                >
                                                                <button
                                                                    class="btn-remove-one"
                                                                    on:click={() =>
                                                                        removeExtractedRequirement(
                                                                            req.id,
                                                                        )}
                                                                >
                                                                    Remove
                                                                </button>
                                                            {/if}
                                                        </div>
                                                    </div>
                                                    {#if allocHint && !req.imported}
                                                        <div
                                                            class="alloc-hint-row"
                                                        >
                                                            <span
                                                                class="alloc-hint-pill conf-{allocHint.confidence}"
                                                            >
                                                                {allocHint.source ===
                                                                "ai"
                                                                    ? "AI"
                                                                    : "Heuristic"}
                                                                suggestion
                                                            </span>
                                                            <span
                                                                class="alloc-hint-text"
                                                            >
                                                                {#if allocHint.newSubsystemName}
                                                                    Suggest new
                                                                    subsystem:
                                                                    <strong
                                                                        >{allocHint.newSubsystemName}</strong
                                                                    >
                                                                {:else}
                                                                    Suggested
                                                                    allocation:
                                                                    <strong
                                                                        >{req.allocation ??
                                                                            "System Level"}</strong
                                                                    >
                                                                {/if}
                                                                {#if allocHint.rationale}
                                                                    - {allocHint.rationale}
                                                                {/if}
                                                            </span>
                                                            {#if allocHint.newSubsystemName}
                                                                <button
                                                                    class="btn-ghost-sm"
                                                                    on:click={() =>
                                                                        createSubsystemFromHint(
                                                                            req.id,
                                                                        )}
                                                                    disabled={allocationCreateLoading ||
                                                                        allocationSuggestLoading ||
                                                                        !canCreateSubsystem()}
                                                                >
                                                                    Create
                                                                    subsystem
                                                                </button>
                                                            {/if}
                                                        </div>
                                                    {/if}
                                                </div>
                                            {/each}
                                        </div>
                                    {/if}
                                </div>
                            {/if}
                        </div>
                    </div>
                {/if}
            {/if}
        </div>
    </div>
</div>

{#if docSwitcherOpen}
    <div
        class="doc-switcher-backdrop"
        on:click={closeDocSwitcher}
        role="presentation"
    >
        <div
            class="doc-switcher"
            role="dialog"
            aria-modal="true"
            aria-label="Document switcher"
            on:click|stopPropagation
        >
            <input
                bind:this={docSwitcherInput}
                class="doc-switcher-input"
                type="text"
                placeholder="Search documents..."
                bind:value={docSwitcherQuery}
            />
            <div class="doc-switcher-list">
                {#if docSwitcherItems.length === 0}
                    <div class="doc-switcher-empty">
                        No documents match "{docSwitcherQuery}".
                    </div>
                {:else}
                    {#each docSwitcherItems as doc, idx (doc.id)}
                        <button
                            class="doc-switcher-item"
                            class:active={idx === docSwitcherIndex}
                            on:mouseenter={() => (docSwitcherIndex = idx)}
                            on:click={() => selectDocFromSwitcher(doc)}
                        >
                            <span class="doc-switcher-name">{doc.name}</span>
                            <span class="doc-switcher-meta"
                                >{doc.doc_type.toUpperCase()} - {fmtSize(
                                    doc.size,
                                )}</span
                            >
                        </button>
                    {/each}
                {/if}
            </div>
            <div class="doc-switcher-hint">
                Enter to open, Esc to close, Up/Down to navigate
            </div>
        </div>
    </div>
{/if}

<style>
    .docs-root {
        display: flex;
        flex-direction: column;
        height: 100%;
        background: var(--surface-base);
        overflow: hidden;
    }
    .docs-root.focus-mode .docs-header {
        display: none;
    }
    .docs-root.is-resizing {
        user-select: none;
    }

    /* Header */
    .docs-header {
        flex-shrink: 0;
        padding: var(--space-2) var(--space-5) !important;
    }

    /* Body layout */
    .docs-body {
        display: grid;
        grid-template-columns: var(--sidebar-width, 300px) 8px minmax(0, 1fr);
        flex: 1;
        overflow: hidden;
        min-height: 0;
    }
    .docs-body.sidebar-collapsed {
        grid-template-columns: minmax(0, 1fr) !important;
    }
    .docs-body.sidebar-collapsed .doc-sidebar,
    .docs-body.sidebar-collapsed .sidebar-resizer {
        display: none !important;
    }

    .pane-resizer {
        position: relative;
        background: var(--surface-border-subtle);
        transition: background var(--transition-fast);
        z-index: 3;
    }
    .pane-resizer:hover {
        background: var(--accent);
    }

    .pane-resizer-vertical {
        cursor: ew-resize;
        width: 8px;
    }

    .sidebar-resizer {
        border-left: 1px solid var(--surface-border);
        border-right: 1px solid var(--surface-border);
    }
    .sidebar-resizer.hidden {
        display: none;
    }

    /* Sidebar */
    .doc-sidebar {
        border-right: 1px solid var(--surface-border);
        background: var(--surface-raised);
        display: flex;
        flex-direction: column;
        overflow: hidden;
        min-height: 0;
    }
    .doc-sidebar.collapsed {
        width: 0;
        min-width: 0;
        border-right: none;
    }
    .doc-sidebar.collapsed > * {
        opacity: 0;
        pointer-events: none;
    }

    .sidebar-header {
        display: flex;
        justify-content: space-between;
        align-items: center;
        padding: var(--space-3);
        border-bottom: 1px solid var(--surface-border);
        flex-shrink: 0;
    }

    .sidebar-actions {
        display: flex;
        align-items: center;
        gap: var(--space-1);
    }

    .sidebar-title {
        font-size: var(--text-xs);
        text-transform: uppercase;
        letter-spacing: 0.12em;
        color: var(--text-muted);
        font-weight: var(--weight-semibold);
    }

    .btn-icon {
        width: 22px;
        height: 22px;
        background: none;
        border: 1px solid var(--surface-border);
        border-radius: var(--radius-sm);
        color: var(--text-muted);
        font-size: 16px;
        cursor: pointer;
        display: flex;
        align-items: center;
        justify-content: center;
        transition: all var(--transition-fast);
    }
    .btn-icon:hover {
        background: var(--surface-hover);
        color: var(--text-primary);
    }

    .drop-zone {
        margin: var(--space-3);
        border: 1px dashed var(--surface-border);
        border-radius: var(--radius-lg);
        padding: var(--space-4) var(--space-3);
        display: flex;
        flex-direction: column;
        align-items: center;
        gap: var(--space-1);
        cursor: pointer;
        transition: all var(--transition-fast);
        flex-shrink: 0;
    }
    .drop-zone:hover,
    .drop-zone.drag-over {
        border-color: var(--accent);
        background: var(--accent-dim);
    }

    .drop-hint {
        font-size: var(--text-sm);
        color: var(--text-secondary);
    }
    .drop-sub {
        font-size: var(--text-xs);
        color: var(--text-muted);
    }

    .doc-list {
        overflow-y: auto;
        flex: 1;
        min-height: 0;
    }

    .doc-empty {
        display: flex;
        flex-direction: column;
        align-items: center;
        gap: var(--space-2);
        padding: var(--space-6) var(--space-4);
        text-align: center;
        font-size: var(--text-sm);
        color: var(--text-muted);
    }
    :global(.doc-empty svg) {
        opacity: 0.4;
    }

    .doc-item {
        display: flex;
        align-items: center;
        gap: var(--space-2);
        width: 100%;
        padding: var(--space-2) var(--space-3);
        background: none;
        border: none;
        text-align: left;
        cursor: pointer;
        transition: background var(--transition-fast);
    }
    .doc-item:hover {
        background: var(--surface-hover);
    }
    .doc-item.active {
        background: var(--accent-dim);
    }

    .doc-icon {
        display: flex;
        align-items: center;
        justify-content: center;
        width: 26px;
        height: 26px;
        flex-shrink: 0;
        color: var(--text-muted);
        background: var(--surface-border-subtle);
        border-radius: var(--radius-sm);
    }
    .doc-item.active .doc-icon {
        color: var(--accent-hover);
        background: var(--accent-dim);
    }

    .doc-item-info {
        flex: 1;
        display: flex;
        flex-direction: column;
        gap: 2px;
        overflow: hidden;
    }

    .doc-item-name {
        font-size: var(--text-sm);
        color: var(--text-secondary);
        white-space: nowrap;
        overflow: hidden;
        text-overflow: ellipsis;
    }
    .doc-item.active .doc-item-name {
        color: var(--accent-hover);
    }

    .doc-item-meta {
        font-size: var(--text-xs);
        color: var(--text-muted);
    }

    .doc-remove {
        background: none;
        border: none;
        color: var(--text-muted);
        font-size: var(--text-xs);
        cursor: pointer;
        padding: 2px 4px;
        border-radius: var(--radius-sm);
        opacity: 0;
        transition: all var(--transition-fast);
    }
    .doc-item:hover .doc-remove {
        opacity: 1;
    }
    .doc-remove:hover {
        color: var(--color-error);
    }

    /* Extractor main */
    .extractor {
        display: flex;
        flex-direction: column;
        overflow: hidden;
        min-height: 0;
        min-width: 0;
    }

    @media (max-width: 1000px) {
        .docs-body {
            grid-template-columns: 1fr;
            grid-template-rows: auto 1fr;
        }
        .sidebar-resizer {
            display: none;
        }
        .doc-sidebar {
            border-right: none;
            border-bottom: 1px solid var(--surface-border);
            max-height: 280px;
        }
    }

    .extractor-empty {
        flex: 1;
        display: flex;
        flex-direction: column;
        align-items: center;
        justify-content: center;
        gap: var(--space-3);
        color: var(--text-muted);
        font-size: var(--text-sm);
    }
    :global(.extractor-empty-icon) {
        color: var(--text-muted);
        opacity: 0.4;
    }
    .extractor-empty-title {
        font-size: var(--text-lg);
        font-weight: var(--weight-semibold);
        color: var(--text-secondary);
        letter-spacing: var(--tracking-tight);
    }
    .extractor-empty-body {
        font-size: var(--text-sm);
        color: var(--text-muted);
        max-width: 300px;
        text-align: center;
        line-height: var(--leading-relaxed);
    }

    .extractor-header {
        display: flex;
        flex-wrap: wrap;
        justify-content: space-between;
        align-items: flex-start;
        padding: var(--space-3) var(--space-5);
        border-bottom: 1px solid var(--surface-border);
        background: var(--surface-raised);
        flex-shrink: 0;
        gap: var(--space-4);
    }

    .extractor-doc-info {
        display: flex;
        flex-direction: column;
        gap: 2px;
        min-width: 0;
        flex: 1 1 260px;
    }

    .extractor-doc-name {
        font-size: var(--text-base);
        font-weight: var(--weight-semibold);
        max-width: 100%;
        white-space: nowrap;
        overflow: hidden;
        text-overflow: ellipsis;
    }

    .extractor-doc-meta {
        font-size: var(--text-xs);
        color: var(--text-muted);
        min-width: 0;
        white-space: nowrap;
        overflow: hidden;
        text-overflow: ellipsis;
    }

    .extractor-actions {
        display: flex;
        align-items: center;
        gap: var(--space-2);
        flex: 1 1 520px;
        min-width: 0;
        flex-wrap: wrap;
        justify-content: flex-end;
    }

    .docs-root.focus-mode .extractor-header {
        padding: var(--space-2) var(--space-3);
        gap: var(--space-2);
    }
    .docs-root.focus-mode .extractor-actions {
        justify-content: flex-start;
        gap: var(--space-1);
    }
    .docs-root.focus-mode .extractor-actions .btn-ghost-sm,
    .docs-root.focus-mode .extractor-actions .btn-primary-sm,
    .docs-root.focus-mode .extractor-actions .btn-ai {
        font-size: var(--text-xs);
        padding: 4px 8px;
    }
    .docs-root.focus-mode .view-toggle-btn {
        padding: 2px 7px;
        font-size: 11px;
    }

    .view-toggle-group {
        display: inline-flex;
        align-items: center;
        gap: 2px;
        padding: 2px;
        border: 1px solid var(--surface-border);
        border-radius: var(--radius-md);
        background: var(--surface-overlay);
    }

    .view-toggle-btn {
        padding: 3px 9px;
        border: none;
        border-radius: var(--radius-sm);
        background: transparent;
        color: var(--text-muted);
        font-size: var(--text-xs);
        cursor: pointer;
        transition: all var(--transition-fast);
    }
    .view-toggle-btn:hover {
        color: var(--text-secondary);
        background: var(--surface-hover);
    }
    .view-toggle-btn.active {
        color: var(--accent-hover);
        background: var(--accent-dim);
        box-shadow: inset 0 0 0 1px var(--accent);
    }

    .requirements-workspace {
        display: flex;
        flex-direction: column;
        flex: 1;
        min-height: 0;
        overflow: hidden;
    }

    .workspace-panels {
        display: flex;
        flex-direction: column;
        flex: 1;
        min-height: 0;
        overflow: hidden;
    }

    .workspace-panels.split {
        display: grid;
        grid-template-columns: minmax(0, 1.2fr) minmax(0, 1fr);
        gap: var(--space-3);
        padding: var(--space-3) var(--space-5) var(--space-4);
    }

    .workspace-panels.split .document-center {
        margin: 0;
    }

    .workspace-panels.split .requirements-panel {
        margin: 0;
        border: 1px solid var(--surface-border);
        border-radius: 14px;
        background: var(--surface-raised);
    }

    .requirements-panel {
        display: flex;
        flex-direction: column;
        flex: 1;
        min-height: 0;
        overflow: hidden;
    }

    .requirements-panel.full-height {
        margin-top: var(--space-1);
    }

    .requirements-panel.split-compact .confidence-filters,
    .requirements-panel.split-compact .allocation-bar,
    .requirements-panel.split-compact .req-count-bar {
        padding: var(--space-1) var(--space-3);
        gap: var(--space-1);
    }

    .requirements-panel.split-compact .req-extract-list {
        padding: var(--space-2) var(--space-3);
        gap: var(--space-1);
    }

    .requirements-panel.split-compact .req-extract-card {
        padding: var(--space-2) var(--space-2);
        border-radius: var(--radius-md);
    }

    .requirements-panel.split-compact .req-extract-top {
        gap: var(--space-2);
    }

    .requirements-panel.split-compact .req-extract-name {
        font-size: 12px;
        line-height: 1.25;
    }

    .requirements-panel.split-compact .req-extract-text {
        font-size: 12px;
        line-height: 1.35;
        display: -webkit-box;
        -webkit-box-orient: vertical;
        -webkit-line-clamp: 4;
        overflow: hidden;
    }

    .requirements-panel.split-compact .req-extract-actions {
        display: flex;
        align-items: center;
        justify-content: flex-end;
        gap: 4px;
        flex-wrap: wrap;
        max-width: 170px;
    }

    .requirements-panel.split-compact .alloc-select {
        max-width: 104px;
        font-size: 10px;
        padding: 1px 5px;
        margin-right: 0;
    }

    .requirements-panel.split-compact .conf-badge {
        font-size: 10px;
        padding: 1px 5px;
    }

    .requirements-panel.split-compact .btn-import-one,
    .requirements-panel.split-compact .btn-remove-one {
        font-size: 10px;
        padding: 2px 6px;
        margin-left: 0;
    }

    .requirements-panel.split-compact .allocation-actions .btn-ghost-sm {
        font-size: 10px;
        padding: 2px 6px;
    }

    .requirements-panel.split-compact .alloc-hint-row {
        gap: 4px;
        margin-top: 4px;
    }

    .requirements-panel.split-compact .alloc-hint-text {
        font-size: 10px;
    }

    .requirements-panel.split-compact .req-flags {
        max-height: 30px;
        overflow: hidden;
    }

    .requirements-panel.split-compact .flag-pill,
    .requirements-panel.split-compact .score-pill {
        font-size: 9px;
        padding: 1px 4px;
    }

    .no-reqs {
        padding: var(--space-8);
        text-align: center;
        color: var(--text-muted);
        font-size: var(--text-sm);
    }

    .req-count-bar {
        display: flex;
        justify-content: space-between;
        padding: var(--space-2) var(--space-5);
        font-size: var(--text-xs);
        color: var(--text-muted);
        border-bottom: 1px solid var(--surface-border);
        flex-shrink: 0;
    }

    .req-count-imported {
        color: var(--color-success);
    }

    .req-extract-list {
        flex: 1;
        overflow-y: auto;
        padding: var(--space-3) var(--space-5);
        display: flex;
        flex-direction: column;
        gap: var(--space-2);
        min-height: 0;
    }

    .document-center {
        margin: var(--space-3) var(--space-5);
        border: 1px solid #2b3549;
        border-radius: 14px;
        background: linear-gradient(180deg, #1b2436 0%, #141b2a 100%);
        box-shadow:
            0 10px 28px rgba(8, 12, 20, 0.36),
            inset 0 1px 0 rgba(255, 255, 255, 0.03);
        display: flex;
        flex-direction: column;
        overflow: hidden;
        flex-shrink: 0;
    }
    .document-center.full-height {
        flex: 1;
        min-height: 0;
    }
    .document-center.full-height .document-center-viewer {
        flex: 1;
        min-height: 0;
    }

    @media (max-width: 900px) {
        .workspace-panels.split {
            grid-template-columns: 1fr;
            gap: var(--space-2);
        }
    }

    @media (max-width: 1280px) {
        .extractor-actions {
            justify-content: flex-start;
        }
    }

    .document-center-head {
        display: flex;
        align-items: flex-start;
        justify-content: space-between;
        gap: var(--space-4);
        padding: var(--space-3) var(--space-4);
        border-bottom: 1px solid #2a3347;
        background: linear-gradient(
            180deg,
            rgba(255, 255, 255, 0.03),
            transparent
        );
    }

    .document-center-title-wrap {
        min-width: 0;
        display: flex;
        flex-direction: column;
        gap: 2px;
    }

    .document-center-title {
        font-size: var(--text-base);
        font-weight: var(--weight-semibold);
        letter-spacing: 0.01em;
        color: #e7edf8;
    }

    .document-center-file {
        font-size: var(--text-xs);
        color: #9bacc6;
        white-space: nowrap;
        overflow: hidden;
        text-overflow: ellipsis;
        max-width: 480px;
    }

    .document-center-meta {
        display: flex;
        align-items: flex-start;
        gap: var(--space-2);
        flex-wrap: wrap;
        justify-content: flex-end;
    }

    .document-meta-pill {
        font-size: 10px;
        letter-spacing: 0.07em;
        text-transform: uppercase;
        color: #9fb0ca;
        border: 1px solid #33415a;
        border-radius: 999px;
        padding: 2px 8px;
        background: rgba(255, 255, 255, 0.02);
        white-space: nowrap;
    }
    .document-meta-pill.active {
        color: #89b8ff;
        border-color: #35538a;
        background: rgba(59, 130, 246, 0.14);
    }
    .document-meta-pill.warning {
        color: #fbbf24;
        border-color: #715021;
        background: rgba(245, 158, 11, 0.14);
    }

    .document-center-help {
        padding: var(--space-2) var(--space-4);
        font-size: var(--text-xs);
        color: #a8b6cb;
        border-bottom: 1px solid #2a3347;
        line-height: 1.5;
        background: rgba(15, 23, 38, 0.5);
    }

    .document-center-viewer {
        overflow: auto;
        padding: 20px 24px;
        font-size: var(--text-sm);
        line-height: var(--leading-relaxed);
        white-space: pre-wrap;
        word-break: break-word;
        background:
            radial-gradient(
                900px 260px at 50% -40px,
                rgba(160, 189, 228, 0.16),
                transparent 65%
            ),
            repeating-linear-gradient(
                45deg,
                transparent 0 24px,
                rgba(255, 255, 255, 0.02) 24px 25px
            ),
            #0e1624;
        color: #dbe7f6;
    }
    .document-center-viewer:focus {
        outline: 2px solid rgba(96, 165, 250, 0.6);
        outline-offset: -2px;
    }

    .document-center-empty {
        display: block;
        max-width: 860px;
        margin: 0 auto;
        padding: 22px 26px;
        border-radius: 10px;
        border: 1px dashed #33435d;
        background: rgba(24, 34, 52, 0.7);
        color: #aab8ce;
        font-style: italic;
    }
    .document-center-empty.viewer-loading {
        display: flex;
        flex-direction: column;
        align-items: center;
        justify-content: center;
        gap: 12px;
        font-style: normal;
        min-height: 120px;
    }

    .review-text-flow {
        max-width: 900px;
        margin: 0 auto;
        padding: 30px 36px;
        border-radius: 10px;
        border: 1px solid #d7d0bf;
        box-shadow:
            0 16px 30px rgba(8, 12, 20, 0.3),
            inset 0 1px 0 rgba(255, 255, 255, 0.6);
        background: linear-gradient(180deg, #fffefa 0%, #fdf9ef 100%);
        color: #1b2432;
        font-family:
            "Source Serif 4", "Palatino Linotype", "Book Antiqua", Georgia,
            serif;
        font-size: 15px;
        line-height: 1.7;
        white-space: pre-wrap;
        word-break: break-word;
    }

    .document-full-text {
        max-width: 900px;
        margin: 0 auto;
        padding: 34px 40px;
        border-radius: 10px;
        border: 1px solid #d7d0bf;
        background: linear-gradient(180deg, #fffefa 0%, #fdf9ef 100%);
        box-shadow:
            0 16px 30px rgba(8, 12, 20, 0.3),
            inset 0 1px 0 rgba(255, 255, 255, 0.6);
        font-size: 15px;
        line-height: 1.7;
        white-space: pre-wrap;
        word-break: break-word;
        color: #1b2432;
        font-family:
            "Source Serif 4", "Palatino Linotype", "Book Antiqua", Georgia,
            serif;
    }

    .pdf-stage {
        width: min(100%, 1140px);
        height: 100%;
        min-height: 320px;
        margin: 0 auto;
        padding: 10px;
        border-radius: 12px;
        border: 1px solid #2b3446;
        background:
            radial-gradient(
                500px 150px at 50% 0,
                rgba(148, 163, 184, 0.2),
                transparent 70%
            ),
            #111827;
        box-shadow: inset 0 0 0 1px rgba(255, 255, 255, 0.03);
        box-sizing: border-box;
    }

    .document-pdf-frame {
        width: 100%;
        height: 100%;
        border: 0;
        border-radius: 8px;
        background: #0d1018;
    }

    .docx-preview-host {
        display: none;
        min-height: 100%;
        max-width: 980px;
        margin: 0 auto;
    }
    .docx-preview-host.active {
        display: block;
    }
    .docx-preview-host :global(.docx-wrapper) {
        background: transparent !important;
        padding: 10px 0 !important;
    }
    .docx-preview-host :global(.docx) {
        color: #111827;
        font-family: Cambria, "Times New Roman", "Liberation Serif", serif;
    }
    .docx-preview-host :global(.docx .docx-page) {
        margin: 0 auto 16px;
        border: 1px solid #d7dce8;
        box-shadow:
            0 14px 28px rgba(12, 17, 28, 0.22),
            0 2px 6px rgba(12, 17, 28, 0.18);
        border-radius: 2px;
        overflow: hidden;
    }

    .docx-rendered {
        max-width: 900px;
        margin: 0 auto;
        color: #1f2937;
        background: #fff;
        border: 1px solid #d7dce8;
        border-radius: 8px;
        padding: 24px;
        min-height: 240px;
        box-sizing: border-box;
        line-height: 1.65;
        white-space: normal;
        word-break: normal;
        box-shadow: 0 12px 24px rgba(12, 17, 28, 0.2);
        font-family: Cambria, "Times New Roman", "Liberation Serif", serif;
    }
    .docx-rendered :global(p) {
        margin: 0 0 0.75rem 0;
    }
    .docx-rendered :global(h1),
    .docx-rendered :global(h2),
    .docx-rendered :global(h3) {
        margin: 1.1rem 0 0.6rem 0;
        color: #0f172a;
    }
    .docx-rendered :global(table) {
        width: 100%;
        border-collapse: collapse;
        margin: 0.75rem 0;
    }
    .docx-rendered :global(td),
    .docx-rendered :global(th) {
        border: 1px solid #cfd6e4;
        padding: 6px 8px;
    }

    .document-render-error {
        max-width: 900px;
        margin: 0 auto var(--space-2);
        box-sizing: border-box;
        padding: var(--space-2);
        border-radius: 8px;
        border: 1px solid #935d1f;
        background: rgba(245, 158, 11, 0.16);
        color: var(--color-warning, #f59e0b);
        font-size: var(--text-xs);
    }

    @media (max-width: 980px) {
        .document-center-head {
            flex-direction: column;
            align-items: flex-start;
        }
        .document-center-file {
            max-width: 100%;
        }
        .document-center-viewer {
            padding: 12px;
        }
        .document-full-text,
        .review-text-flow,
        .docx-rendered {
            padding: 20px 18px;
        }
    }

    .pane-resizer-horizontal {
        cursor: ns-resize;
        height: 8px;
        width: 100%;
    }

    .viewer-resizer {
        border-top: 1px solid var(--surface-border);
        border-bottom: 1px solid var(--surface-border);
    }

    .req-highlight {
        padding: 1px 3px;
        border-radius: 4px;
        cursor: pointer;
        transition:
            filter var(--transition-fast),
            box-shadow var(--transition-fast);
        box-decoration-break: clone;
        -webkit-box-decoration-break: clone;
    }
    .req-highlight:hover {
        filter: brightness(1.05);
        box-shadow: 0 0 0 1px rgba(15, 23, 42, 0.15);
    }
    .req-highlight.conf-high {
        background: #22c55e33;
        color: #166534;
    }
    .req-highlight.conf-medium {
        background: #f59e0b3a;
        color: #b45309;
    }
    .req-highlight.conf-low {
        background: #94a3b840;
        color: #334155;
    }

    .highlight-review-selection {
        display: flex;
        align-items: center;
        gap: var(--space-2);
        padding: var(--space-2) var(--space-3);
        border-top: 1px solid var(--surface-border);
        background: var(--surface-base);
        flex-wrap: wrap;
    }

    .selection-label {
        font-size: var(--text-xs);
        color: var(--text-muted);
        flex-shrink: 0;
    }

    .selection-preview {
        font-size: var(--text-xs);
        color: var(--text-secondary);
        background: var(--surface-overlay);
        border: 1px solid var(--surface-border);
        border-radius: var(--radius-sm);
        padding: 2px 6px;
        flex: 1;
        min-width: 180px;
    }

    .highlight-review-note {
        padding: var(--space-2) var(--space-3);
        font-size: var(--text-xs);
        color: var(--text-muted);
        border-top: 1px solid var(--surface-border);
        background: var(--surface-base);
    }

    .req-extract-card {
        background: var(--surface-raised);
        border: 1px solid var(--surface-border);
        border-radius: var(--radius-lg);
        padding: var(--space-3) var(--space-4);
        transition: border-color var(--transition-fast);
    }
    .req-extract-card:hover {
        border-color: var(--accent);
    }
    .req-extract-card.selected {
        border-color: var(--accent);
        background: var(--accent-dim);
    }
    .req-extract-card.imported {
        opacity: 0.5;
    }

    .req-extract-top {
        display: flex;
        align-items: flex-start;
        gap: var(--space-3);
    }

    .req-extract-check {
        flex-shrink: 0;
        margin-top: 2px;
        cursor: pointer;
    }

    .req-extract-text {
        flex: 1;
        font-size: var(--text-sm);
        color: var(--text-secondary);
        line-height: 1.6;
        margin: 0;
    }

    .req-extract-actions {
        flex-shrink: 0;
    }
    .alloc-hint-row {
        margin-top: var(--space-2);
        padding-top: var(--space-2);
        border-top: 1px dashed var(--surface-border);
        display: flex;
        align-items: center;
        gap: var(--space-2);
        flex-wrap: wrap;
    }
    .alloc-hint-pill {
        font-size: 10px;
        border: 1px solid var(--surface-border);
        border-radius: 999px;
        padding: 1px 6px;
        color: var(--text-muted);
        background: var(--surface-overlay);
    }
    .alloc-hint-pill.conf-high {
        color: #166534;
        border-color: #22c55e88;
        background: #22c55e1f;
    }
    .alloc-hint-pill.conf-medium {
        color: #b45309;
        border-color: #f59e0b7a;
        background: #f59e0b1f;
    }
    .alloc-hint-pill.conf-low {
        color: var(--text-muted);
    }
    .alloc-hint-text {
        font-size: var(--text-xs);
        color: var(--text-muted);
        flex: 1;
        min-width: 180px;
    }
    .alloc-select {
        background: var(--surface-overlay);
        border: 1px solid var(--surface-border);
        border-radius: var(--radius-sm);
        color: var(--text-secondary);
        font-size: var(--text-xs);
        padding: 2px var(--space-2);
        margin-right: var(--space-1);
    }
    .alloc-select:focus {
        outline: none;
        border-color: var(--accent);
    }

    .imported-badge {
        font-size: var(--text-xs);
        color: var(--color-success);
        padding: 2px 6px;
        border-radius: 999px;
        border: 1px solid var(--color-success);
    }

    .btn-import-one {
        padding: var(--space-1) var(--space-3);
        background: var(--accent-dim);
        border: 1px solid var(--accent);
        border-radius: var(--radius-md);
        color: var(--accent-hover);
        font-size: var(--text-xs);
        cursor: pointer;
        transition: all var(--transition-fast);
        white-space: nowrap;
    }
    .btn-import-one:hover:not(:disabled) {
        background: var(--accent);
        color: #fff;
    }
    .btn-import-one:disabled {
        opacity: 0.4;
        cursor: not-allowed;
    }

    .btn-remove-one {
        margin-left: var(--space-1);
        padding: var(--space-1) var(--space-2);
        background: none;
        border: 1px solid var(--surface-border);
        border-radius: var(--radius-sm);
        color: var(--text-muted);
        font-size: var(--text-xs);
        cursor: pointer;
        transition: all var(--transition-fast);
    }
    .btn-remove-one:hover {
        border-color: var(--color-error, #ef4444);
        color: var(--color-error, #ef4444);
        background: #ef444418;
    }

    /* Shared buttons */
    .btn-primary-sm {
        padding: var(--space-1) var(--space-4);
        background: var(--accent);
        color: #fff;
        border: none;
        border-radius: var(--radius-md);
        font-size: var(--text-sm);
        font-weight: var(--weight-medium);
        cursor: pointer;
        transition: background var(--transition-fast);
    }
    .btn-primary-sm:hover:not(:disabled) {
        background: var(--accent-hover);
    }
    .btn-primary-sm:disabled {
        opacity: 0.4;
        cursor: not-allowed;
    }

    .btn-ghost-sm {
        padding: var(--space-1) var(--space-3);
        background: none;
        border: 1px solid var(--surface-border);
        border-radius: var(--radius-md);
        color: var(--text-secondary);
        font-size: var(--text-sm);
        cursor: pointer;
        transition: all var(--transition-fast);
    }
    .btn-ghost-sm:hover {
        background: var(--surface-hover);
        color: var(--text-primary);
    }
    .btn-ghost-sm.focus-active {
        border-color: #22c55e66;
        background: #22c55e14;
        color: #22c55e;
    }

    .graphrag-toggle {
        display: flex;
        align-items: center;
        gap: 6px;
        padding: 2px var(--space-2);
        border: 1px solid var(--surface-border);
        border-radius: 999px;
        background: var(--surface-overlay);
        color: var(--text-secondary);
        font-size: var(--text-xs);
        user-select: none;
        white-space: nowrap;
    }
    .graphrag-toggle input {
        margin: 0;
        accent-color: var(--accent);
    }
    .graphrag-toggle.active {
        border-color: #22c55e66;
        background: #22c55e14;
        color: #22c55e;
    }

    /* Sidecar banner */
    .sidecar-banner {
        display: flex;
        align-items: center;
        gap: 6px;
        margin: 4px var(--space-5) 0;
        padding: 4px 10px;
        width: fit-content;
        max-width: calc(100% - var(--space-5) - var(--space-5));
        border: 1px solid transparent;
        border-radius: 999px;
        font-size: var(--text-xs);
        line-height: 1.2;
        flex-shrink: 0;
    }
    .sidecar-banner.analyzing {
        color: var(--accent-hover);
        background: #3b82f61a;
        border-color: #3b82f640;
    }
    .sidecar-banner.ok {
        color: var(--color-success);
        background: #22c55e18;
        border-color: #22c55e40;
    }
    .sidecar-banner.ai-mode {
        color: #a78bfa;
        background: #a78bfa18;
        border-color: #a78bfa40;
    }
    .sidecar-banner.warning {
        color: var(--color-warning, #f59e0b);
        background: #f59e0b18;
        border-color: #f59e0b40;
    }

    /* AI button */
    .btn-ai {
        display: flex;
        align-items: center;
        gap: var(--space-1);
        padding: var(--space-1) var(--space-3);
        background: linear-gradient(135deg, #7c3aed22, #4f46e522);
        border: 1px solid #7c3aed55;
        border-radius: var(--radius-md);
        color: #a78bfa;
        font-size: var(--text-sm);
        font-weight: var(--weight-medium);
        cursor: pointer;
        transition: all var(--transition-fast);
        white-space: nowrap;
    }
    .btn-ai:hover:not(:disabled):not(.unavailable) {
        background: linear-gradient(135deg, #7c3aed44, #4f46e544);
        border-color: #7c3aed99;
        color: #c4b5fd;
    }
    .btn-ai:disabled {
        opacity: 0.5;
        cursor: not-allowed;
    }
    .btn-ai.unavailable {
        opacity: 0.4;
        cursor: not-allowed;
        filter: grayscale(0.5);
    }
    .btn-spinner {
        display: inline-block;
        animation: spin 1s linear infinite;
    }
    .error-detail {
        display: block;
        margin-top: var(--space-1);
        font-size: 10px;
        opacity: 0.8;
        word-break: break-all;
    }

    .banner-spinner {
        display: inline-block;
        width: 12px;
        height: 12px;
        border: 1.5px solid currentColor;
        border-top-color: transparent;
        border-radius: 50%;
        animation: spin 0.75s linear infinite;
        flex-shrink: 0;
        opacity: 0.7;
    }
    @keyframes spin {
        to {
            transform: rotate(360deg);
        }
    }

    /* Confidence filter toggles */
    .confidence-filters {
        display: flex;
        align-items: center;
        gap: var(--space-2);
        padding: var(--space-2) var(--space-5);
        border-bottom: 1px solid var(--surface-border);
        flex-shrink: 0;
    }

    .allocation-bar {
        display: flex;
        align-items: center;
        gap: var(--space-2);
        padding: var(--space-2) var(--space-5);
        border-bottom: 1px solid var(--surface-border);
        flex-shrink: 0;
        flex-wrap: wrap;
    }

    .allocation-actions {
        display: flex;
        gap: var(--space-1);
        margin-left: auto;
    }

    .alloc-pill {
        font-size: var(--text-xs);
        padding: 2px var(--space-2);
        border-radius: 999px;
        border: 1px solid var(--surface-border);
        color: var(--text-secondary);
        background: var(--surface-overlay);
    }

    .filter-label {
        font-size: var(--text-xs);
        color: var(--text-muted);
        text-transform: uppercase;
        letter-spacing: 0.08em;
    }

    .conf-toggle {
        padding: 2px var(--space-3);
        border-radius: 999px;
        border: 1px solid var(--surface-border);
        background: none;
        font-size: var(--text-xs);
        cursor: pointer;
        color: var(--text-muted);
        transition: all var(--transition-fast);
        opacity: 0.5;
    }
    .conf-toggle.active {
        opacity: 1;
    }
    .conf-toggle.high.active {
        color: #22c55e;
        border-color: #22c55e;
        background: #22c55e18;
    }
    .conf-toggle.medium.active {
        color: #f59e0b;
        border-color: #f59e0b;
        background: #f59e0b18;
    }
    .conf-toggle.low.active {
        color: var(--text-muted);
        border-color: var(--surface-border);
    }
    .conf-toggle.dupe.active {
        color: #8888a8;
        border-color: #8888a8;
        background: #8888a818;
    }

    /* Card confidence left border */
    .req-extract-card.conf-high {
        border-left: 3px solid #22c55e;
    }
    .req-extract-card.conf-medium {
        border-left: 3px solid #f59e0b;
    }
    .req-extract-card.conf-low {
        border-left: 3px solid var(--surface-border);
    }
    .req-extract-card.is-dupe {
        opacity: 0.55;
    }

    /* Name + body layout inside card */
    .req-extract-body {
        flex: 1;
        display: flex;
        flex-direction: column;
        gap: var(--space-1);
    }

    .req-extract-name {
        font-size: var(--text-sm);
        font-weight: var(--weight-semibold);
        color: var(--text-primary);
        display: flex;
        align-items: center;
        gap: var(--space-2);
    }

    .dupe-badge {
        font-size: var(--text-xs);
        color: #8888a8;
        border: 1px solid #8888a840;
        padding: 1px 6px;
        border-radius: 999px;
    }

    /* Flags / score pills */
    .req-flags {
        display: flex;
        flex-wrap: wrap;
        gap: 4px;
        margin-top: var(--space-1);
    }

    .flag-pill {
        font-size: 10px;
        font-family: var(--font-mono);
        color: var(--text-muted);
        background: var(--surface-overlay);
        border: 1px solid var(--surface-border);
        padding: 1px 5px;
        border-radius: var(--radius-sm);
    }

    .score-pill {
        font-size: 10px;
        font-family: var(--font-mono);
        color: var(--accent-hover);
        background: var(--accent-dim);
        border: 1px solid var(--accent);
        padding: 1px 5px;
        border-radius: var(--radius-sm);
    }

    /* Export dropdown */
    .export-wrap {
        position: relative;
    }

    .export-menu {
        position: absolute;
        top: calc(100% + 4px);
        right: 0;
        background: var(--surface-overlay);
        border: 1px solid var(--surface-border);
        border-radius: var(--radius-lg);
        box-shadow: var(--shadow-lg);
        min-width: 210px;
        z-index: 100;
        display: flex;
        flex-direction: column;
        overflow: hidden;
    }

    .export-item {
        display: flex;
        align-items: center;
        gap: var(--space-3);
        padding: var(--space-2) var(--space-4);
        background: none;
        border: none;
        text-align: left;
        cursor: pointer;
        transition: background var(--transition-fast);
    }
    .export-item:hover {
        background: var(--surface-hover);
    }

    .export-icon {
        font-size: 14px;
        color: var(--text-muted);
        flex-shrink: 0;
    }

    .export-item strong {
        display: block;
        font-size: var(--text-sm);
        color: var(--text-primary);
        font-weight: var(--weight-medium);
    }

    .export-desc {
        display: block;
        font-size: var(--text-xs);
        color: var(--text-muted);
    }

    /* Confidence badge on card actions */
    .conf-badge {
        font-size: var(--text-xs);
        padding: 1px 7px;
        border-radius: 999px;
        border: 1px solid;
        white-space: nowrap;
    }
    .conf-badge-high {
        color: #22c55e;
        border-color: #22c55e40;
        background: #22c55e18;
    }
    .conf-badge-medium {
        color: #f59e0b;
        border-color: #f59e0b40;
        background: #f59e0b18;
    }
    .conf-badge-low {
        color: var(--text-muted);
        border-color: var(--surface-border);
    }

    /* Doc type selector */
    .extractor-doc-meta-row {
        display: flex;
        align-items: center;
        gap: var(--space-3);
        min-width: 0;
        flex-wrap: wrap;
    }

    .doc-type-select {
        background: var(--surface-overlay);
        border: 1px solid var(--surface-border);
        border-radius: var(--radius-sm);
        color: var(--text-secondary);
        font-size: var(--text-xs);
        padding: 1px var(--space-2);
        cursor: pointer;
        transition: border-color var(--transition-fast);
    }
    .doc-type-select:focus {
        outline: none;
        border-color: var(--accent);
    }

    /* Tab bar */
    .doc-tab-bar {
        display: flex;
        align-items: center;
        gap: 0;
        border-bottom: 1px solid var(--surface-border);
        background: var(--surface-raised);
        flex-shrink: 0;
        padding: 0 var(--space-5);
    }
    .docs-root.focus-mode .doc-tab-bar {
        padding: 0 var(--space-3);
    }

    .doc-tab {
        display: flex;
        align-items: center;
        gap: var(--space-1);
        padding: var(--space-2) var(--space-3);
        background: none;
        border: none;
        border-bottom: 2px solid transparent;
        font-size: var(--text-sm);
        color: var(--text-muted);
        cursor: pointer;
        transition: all var(--transition-fast);
        margin-bottom: -1px;
    }
    .doc-tab:hover {
        color: var(--text-secondary);
    }
    .doc-tab.active {
        color: var(--accent-hover);
        border-bottom-color: var(--accent);
    }

    .tab-count {
        font-size: 10px;
        background: var(--surface-overlay);
        border: 1px solid var(--surface-border);
        border-radius: 999px;
        padding: 0 5px;
        color: var(--text-muted);
        line-height: 1.6;
    }
    .doc-tab.active .tab-count {
        background: var(--accent-dim);
        border-color: var(--accent);
        color: var(--accent-hover);
    }

    /* Sections panel */
    .sections-panel {
        flex: 1;
        display: flex;
        flex-direction: column;
        overflow: hidden;
        min-height: 0;
    }

    .sections-toolbar {
        display: flex;
        align-items: center;
        justify-content: space-between;
        gap: var(--space-4);
        padding: var(--space-3) var(--space-5);
        border-bottom: 1px solid var(--surface-border);
        flex-shrink: 0;
    }

    .sections-hint {
        font-size: var(--text-xs);
        color: var(--text-muted);
        flex: 1;
    }

    .sections-empty {
        flex: 1;
        display: flex;
        flex-direction: column;
        align-items: center;
        justify-content: center;
        gap: var(--space-2);
        color: var(--text-muted);
        font-size: var(--text-sm);
        padding: var(--space-8);
        text-align: center;
    }
    :global(.sections-empty svg) {
        opacity: 0.25;
    }
    .sections-empty-sub {
        font-size: var(--text-xs);
        color: var(--text-muted);
        opacity: 0.7;
        max-width: 280px;
    }

    .section-list {
        flex: 1;
        overflow-y: auto;
        padding: var(--space-3) var(--space-5);
        display: flex;
        flex-direction: column;
        gap: var(--space-2);
        min-height: 0;
    }

    .section-card {
        background: var(--surface-raised);
        border: 1px solid var(--surface-border);
        border-left: 3px solid var(--surface-border);
        border-radius: var(--radius-lg);
        padding: var(--space-2) var(--space-3);
        transition: border-color var(--transition-fast);
    }
    .section-card:hover {
        border-color: var(--accent);
        border-left-color: var(--accent);
    }
    .section-card.type-heading {
        border-left-color: var(--accent);
    }
    .section-card.type-requirement {
        border-left-color: #22c55e;
    }
    .section-card.type-bom_item {
        border-left-color: #f59e0b;
    }
    .section-card.type-sow_section {
        border-left-color: #818cf8;
    }
    .section-card.type-list_item {
        border-left-color: var(--surface-border);
    }

    .section-card-top {
        display: flex;
        align-items: center;
        gap: var(--space-2);
        flex-wrap: wrap;
    }

    .section-type-badge {
        font-size: 10px;
        text-transform: uppercase;
        letter-spacing: 0.08em;
        color: var(--text-muted);
        background: var(--surface-overlay);
        border: 1px solid var(--surface-border);
        border-radius: var(--radius-sm);
        padding: 1px 5px;
        flex-shrink: 0;
    }

    .section-ref {
        font-size: var(--text-xs);
        font-family: var(--font-mono);
        color: var(--accent-hover);
        flex-shrink: 0;
    }

    .section-title {
        font-size: var(--text-sm);
        color: var(--text-primary);
        font-weight: var(--weight-medium);
        flex: 1;
        white-space: nowrap;
        overflow: hidden;
        text-overflow: ellipsis;
    }

    .section-body {
        font-size: var(--text-xs);
        color: var(--text-muted);
        margin: var(--space-1) 0 0;
        line-height: var(--leading-relaxed);
        white-space: pre-wrap;
        word-break: break-word;
    }

    .section-actions {
        display: flex;
        align-items: center;
        gap: var(--space-1);
        flex-shrink: 0;
        margin-left: auto;
    }

    .linked-badge {
        display: flex;
        align-items: center;
        gap: 3px;
        font-size: 10px;
        color: #22c55e;
        background: #22c55e18;
        border: 1px solid #22c55e40;
        border-radius: 999px;
        padding: 1px 6px;
    }

    .btn-link-req {
        display: flex;
        align-items: center;
        gap: 4px;
        padding: 2px var(--space-2);
        background: none;
        border: 1px solid var(--surface-border);
        border-radius: var(--radius-sm);
        font-size: var(--text-xs);
        color: var(--text-muted);
        cursor: pointer;
        transition: all var(--transition-fast);
        opacity: 0;
    }
    .section-card:hover .btn-link-req {
        opacity: 1;
    }
    .btn-link-req:hover {
        border-color: var(--accent);
        color: var(--accent-hover);
        background: var(--accent-dim);
    }

    /* Link-to-req picker */
    .link-req-picker {
        display: flex;
        align-items: center;
        gap: var(--space-2);
        margin-top: var(--space-2);
        padding: var(--space-2) var(--space-3);
        background: var(--surface-overlay);
        border-radius: var(--radius-md);
        border: 1px solid var(--accent);
        flex-wrap: wrap;
    }

    .picker-label {
        font-size: var(--text-xs);
        color: var(--text-muted);
        white-space: nowrap;
    }

    .picker-empty {
        font-size: var(--text-xs);
        color: var(--text-muted);
        font-style: italic;
    }

    .picker-select {
        flex: 1;
        min-width: 200px;
        background: var(--surface-base);
        border: 1px solid var(--surface-border);
        border-radius: var(--radius-sm);
        color: var(--text-secondary);
        font-size: var(--text-xs);
        padding: var(--space-1) var(--space-2);
    }
    .picker-select:focus {
        outline: none;
        border-color: var(--accent);
    }

    .doc-switcher-backdrop {
        position: fixed;
        inset: 0;
        background: rgba(2, 6, 23, 0.62);
        backdrop-filter: blur(3px);
        z-index: 1200;
        display: flex;
        align-items: flex-start;
        justify-content: center;
        padding: 12vh var(--space-4) var(--space-4);
    }

    .doc-switcher {
        width: min(720px, 100%);
        background: var(--surface-overlay);
        border: 1px solid var(--surface-border);
        border-radius: 14px;
        box-shadow: var(--shadow-lg);
        overflow: hidden;
        display: flex;
        flex-direction: column;
        max-height: 72vh;
    }

    .doc-switcher-input {
        width: 100%;
        border: none;
        border-bottom: 1px solid var(--surface-border);
        background: transparent;
        color: var(--text-primary);
        padding: var(--space-3) var(--space-4);
        font-size: var(--text-base);
    }
    .doc-switcher-input:focus {
        outline: none;
        border-bottom-color: var(--accent);
    }

    .doc-switcher-list {
        overflow-y: auto;
        padding: var(--space-2);
        display: flex;
        flex-direction: column;
        gap: 2px;
    }

    .doc-switcher-empty {
        padding: var(--space-4);
        font-size: var(--text-sm);
        color: var(--text-muted);
        text-align: center;
    }

    .doc-switcher-item {
        width: 100%;
        background: transparent;
        border: 1px solid transparent;
        border-radius: var(--radius-md);
        padding: var(--space-2) var(--space-3);
        display: flex;
        align-items: baseline;
        justify-content: space-between;
        gap: var(--space-3);
        cursor: pointer;
        text-align: left;
    }
    .doc-switcher-item:hover {
        background: var(--surface-hover);
    }
    .doc-switcher-item.active {
        border-color: var(--accent);
        background: var(--accent-dim);
    }

    .doc-switcher-name {
        font-size: var(--text-sm);
        color: var(--text-primary);
        white-space: nowrap;
        overflow: hidden;
        text-overflow: ellipsis;
    }

    .doc-switcher-meta {
        font-size: var(--text-xs);
        color: var(--text-muted);
        white-space: nowrap;
    }

    .doc-switcher-hint {
        border-top: 1px solid var(--surface-border);
        padding: var(--space-2) var(--space-3);
        font-size: var(--text-xs);
        color: var(--text-muted);
        text-align: center;
    }
</style>
