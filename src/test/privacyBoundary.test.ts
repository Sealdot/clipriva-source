import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, it } from "vitest";

const requiredBoundaryDocuments = [
  resolve(process.cwd(), "PRIVACY.md"),
  resolve(process.cwd(), "docs/privacy/data-flow.md"),
  resolve(process.cwd(), "docs/open-source/module-boundary.md"),
];

const localLinkV09Documents = [
  resolve(process.cwd(), "docs/product/local-link-v09.md"),
  resolve(process.cwd(), "docs/design/local-link-v09.md"),
  resolve(process.cwd(), "docs/design/local-link-v09-state-matrix.md"),
  resolve(process.cwd(), "docs/security/local-link-boundary.md"),
  resolve(process.cwd(), "docs/qa/local-link-v09-matrix.md"),
  resolve(process.cwd(), "docs/release/local-link-beta-evidence.md"),
  resolve(process.cwd(), "docs/clipriva-0.9-implementation-and-acceptance.md"),
];

const sourceFile = (relativePath: string) =>
  readFileSync(resolve(process.cwd(), relativePath), "utf8");

const sourceSection = (source: string, start: string, end: string) => {
  const startIndex = source.indexOf(start);
  const endIndex = source.indexOf(end, startIndex + start.length);
  expect(startIndex, `Missing source marker: ${start}`).toBeGreaterThanOrEqual(0);
  expect(endIndex, `Missing source marker: ${end}`).toBeGreaterThan(startIndex);
  return source.slice(startIndex, endIndex);
};

const readBoundaryDocuments = () =>
  requiredBoundaryDocuments.map((path) => ({
    path,
    content: readFileSync(path, "utf8"),
  }));

const readLocalLinkV09Documents = () =>
  localLinkV09Documents.map((path) => ({
    path,
    content: readFileSync(path, "utf8"),
  }));

describe("v1 documented privacy boundary", () => {
  it("keeps Item, Occurrence, and Variant storage explicit in every required boundary document", () => {
    for (const document of readBoundaryDocuments()) {
      expect(document.content, document.path).toMatch(/\bItem\b/);
      expect(document.content, document.path).toMatch(/\bOccurrence\b/);
      expect(document.content, document.path).toMatch(/\bVariant\b/);
    }
  });

  it("documents progressive Accessibility permission and a clipboard-only fallback", () => {
    for (const document of readBoundaryDocuments()) {
      expect(document.content, document.path).toMatch(
        /direct(?:-|\s+)paste[\s\S]{0,180}first|first[\s\S]{0,180}direct(?:-|\s+)paste/i,
      );
      expect(document.content, document.path).toMatch(/clipboard/i);
      expect(document.content, document.path).toMatch(
        /without trust|denied|revoked|unavailable|absent/i,
      );
    }
  });

  it("keeps pre-persistence privacy policy broader than clipboard body storage", () => {
    for (const document of readBoundaryDocuments()) {
      expect(document.content, document.path).toMatch(
        /(before[\s\S]{0,180}(Item|Occurrence|Variant)[\s\S]{0,180}(persist|write)|(Item|Occurrence|Variant)[\s\S]{0,180}before[\s\S]{0,180}(persist|write))/i,
      );
    }
  });

  it("documents bounded local tags without creating a Local Link or diagnostics path", () => {
    for (const document of readBoundaryDocuments()) {
      expect(document.content, document.path).toMatch(/user tags?|用户标签/i);
      expect(document.content, document.path).toMatch(/eight|8|八/);
      expect(document.content, document.path).toMatch(/24/);
      expect(document.content, document.path).toMatch(/SQLite/i);
      expect(document.content, document.path).toMatch(
        /tag[s\s\S]{0,220}(never|does not|不)[\s\S]{0,80}(Local Link|diagnostic)/i,
      );
    }
    const models = sourceFile("src-tauri/src/models.rs");
    const localLinkPayload = sourceSection(
      models,
      "pub(crate) struct LocalLinkClipboardPayload",
      "pub enum LocalLinkDiagnosticOsFamily",
    );
    expect(localLinkPayload).not.toMatch(/tags?\s*:/i);
  });

  it("documents explicit default-off native Local Link lifecycle", () => {
    for (const document of readBoundaryDocuments()) {
      expect(document.content, document.path).toMatch(/Local Link/i);
      expect(document.content, document.path).toMatch(/default(?:-|\s+)off/i);
      expect(document.content, document.path).toMatch(/Bonjour|mDNS/i);
      expect(document.content, document.path).toMatch(/Noise/i);
      expect(document.content, document.path).toMatch(/TCP/i);
      expect(document.content, document.path).toMatch(/fail(?:-|\s*)closed/i);
      expect(document.content, document.path).toMatch(/explicit(?:ly)?|only explicit/i);
      expect(document.content, document.path).toMatch(/listener|mDNS|network|TCP/i);
      expect(document.content, document.path).toMatch(/disable|reset|exit/i);
      expect(document.content, document.path).toMatch(/stop|clear|remove|close/i);
    }
  });

  it("keeps the Local Link native identity in Keychain and outside SQLite and the WebView", () => {
    for (const document of readBoundaryDocuments()) {
      expect(document.content, document.path).toMatch(/Keychain/i);
      expect(document.content, document.path).toMatch(/private (?:identity|key|secret)/i);
      expect(document.content, document.path).toMatch(
        /(not|never|no)[\s\S]{0,120}(SQLite|WebView|IPC|log|diagnostic)/i,
      );
      expect(document.content, document.path).toMatch(/reset|rotation|re-pair/i);
    }
  });

  it("documents the sixty-second memory-only receiver decision and exclusive destinations", () => {
    for (const document of readBoundaryDocuments()) {
      expect(document.content, document.path).toMatch(
        /60[\s\S]{0,80}(second|deadline)|(?:native|process)[\s\S]{0,100}memory/i,
      );
      expect(document.content, document.path).toMatch(/Copy[\s\S]{0,180}(clipboard|system)/i);
      expect(document.content, document.path).toMatch(/Save[\s\S]{0,180}History/i);
      expect(document.content, document.path).toMatch(/Reject[\s\S]{0,180}(neither|clear|write)/i);
      expect(document.content, document.path).toMatch(/cancel|expiry|revoke/i);
    }
  });

  it("keeps Local Link free of cloud relay, offline queues, plaintext fallback, and Beta overclaim", () => {
    for (const document of readBoundaryDocuments()) {
      expect(document.content, document.path).toMatch(
        /no[\s\S]{0,40}(cloud|Internet)[\s\S]{0,20}relay|has no[\s\S]{0,40}cloud relay/i,
      );
      expect(document.content, document.path).toMatch(
        /no[\s\S]{0,100}offline[\s\S]{0,20}(queue|delivery)|has no[\s\S]{0,100}offline queue/i,
      );
      expect(document.content, document.path).toMatch(/plaintext[\s\S]{0,80}(fallback|codec)/i);
      expect(document.content, document.path).toMatch(/Preview|Alpha/i);
      expect(document.content, document.path).toMatch(
        /50[\s\S]{0,80}200|50-pairing[\s\S]{0,80}200-transfer/i,
      );
    }
  });

  it("declares the bundled local-network purpose and Bonjour service", () => {
    const infoPlist = sourceFile("src-tauri/Info.plist");
    expect(infoPlist).toMatch(/NSLocalNetworkUsageDescription/);
    expect(infoPlist).toMatch(/_clipriva\._tcp/);
    for (const document of readBoundaryDocuments()) {
      expect(document.content, document.path).toMatch(/Local Network/i);
      expect(document.content, document.path).toMatch(/_clipriva\._tcp/);
    }
  });

  it("keeps Local Link transfer DTOs content-free on both sides of IPC", () => {
    const types = sourceFile("src/features/clipboard/types.ts");
    const models = sourceFile("src-tauri/src/models.rs");
    const webviewTransfer = sourceSection(
      types,
      "export interface LocalLinkTransfer {",
      "  isUiFixture?: boolean;",
    );
    const nativeTransfer = sourceSection(
      models,
      "pub struct LocalLinkTransfer {",
      "pub(crate) struct LocalLinkClipboardPayload",
    );

    for (const transferDto of [webviewTransfer, nativeTransfer]) {
      expect(transferDto).not.toMatch(/\bcontent\s*[:]/i);
      expect(transferDto).not.toMatch(/\bpreview\s*[:]/i);
      expect(transferDto).not.toMatch(/\bdigest\s*[:]/i);
      expect(transferDto).not.toMatch(/\b(?:ip|port|endpoint)\s*[:]/i);
    }
  });

  it("makes reveal an opaque native intent and registers reset and summary cleanup", () => {
    const api = sourceFile("src/features/clipboard/api.ts");
    const commands = sourceFile("src-tauri/src/commands.rs");
    const commandRegistry = sourceFile("src-tauri/src/lib.rs");
    const revealApi = sourceSection(
      api,
      "export async function revealLocalLinkTransfer",
      "export async function resetLocalLinkIdentity",
    );
    const revealCommand = sourceSection(
      commands,
      "pub fn reveal_local_link_transfer",
      "pub fn cancel_local_link_transfer",
    );

    expect(revealApi).toMatch(/invoke<void>\("reveal_local_link_transfer"/);
    expect(revealApi).not.toMatch(/LocalLinkClipboardPayload|content|preview|body/i);
    expect(revealCommand).toMatch(/CommandResult<\(\)>/);
    expect(revealCommand).not.toMatch(/CommandResult<\s*(?:String|LocalLinkClipboardPayload)/);
    for (const command of [
      "reveal_local_link_transfer",
      "reset_local_link_identity",
      "clear_local_link_transfers",
    ]) {
      expect(commandRegistry).toContain(`commands::${command}`);
    }
  });

  it("keeps Incoming Center viewed as a named, metadata-only IPC transition", () => {
    const api = sourceFile("src/features/clipboard/api.ts");
    const commands = sourceFile("src-tauri/src/commands.rs");
    const commandRegistry = sourceFile("src-tauri/src/lib.rs");
    const viewedApi = sourceSection(
      api,
      "export async function markLocalLinkTransferViewed",
      "export async function acceptLocalLinkTransfer",
    );
    const viewedCommand = sourceSection(
      commands,
      "pub fn mark_local_link_transfer_viewed",
      "pub fn cancel_local_link_transfer",
    );

    expect(viewedApi).toMatch(/invoke<LocalLinkTransfer>\("mark_local_link_transfer_viewed"/);
    expect(viewedApi).toMatch(/\{ transferId \}/);
    expect(viewedApi).not.toMatch(/LocalLinkClipboardPayload|invoke<(?:string|void)>/);
    expect(viewedCommand).toMatch(/CommandResult<LocalLinkTransfer>/);
    expect(viewedCommand).not.toMatch(/LocalLinkClipboardPayload|CommandResult<String>/);
    expect(commandRegistry).toContain("commands::mark_local_link_transfer_viewed");
  });

  it("keeps Local Link readiness passive and diagnostics explicitly bounded and content-free", () => {
    const native = sourceFile("src-tauri/src/local_link.rs");
    const models = sourceFile("src-tauri/src/models.rs");
    const commandRegistry = sourceFile("src-tauri/src/lib.rs");
    const diagnosticsDto = sourceSection(
      models,
      "pub struct LocalLinkDiagnostics {",
      "    pub transfers: Vec<LocalLinkDiagnosticTransfer>,",
    );

    expect(native).toContain("MAX_DIAGNOSTIC_RECORDS: usize = 50");
    expect(native).toMatch(/readiness_snapshot[\s\S]{0,800}transport/);
    expect(diagnosticsDto).not.toMatch(
      /content|preview|display_name|device_id|fingerprint|endpoint|byte_size|clipboard_item_id/i,
    );
    for (const command of ["get_local_link_readiness_snapshot", "build_local_link_diagnostics"]) {
      expect(commandRegistry).toContain(`commands::${command}`);
    }
    for (const document of readBoundaryDocuments()) {
      expect(document.content, document.path).toMatch(/readiness|就绪/i);
      expect(document.content, document.path).toMatch(/50/);
      expect(document.content, document.path).toMatch(
        /not stored|does not persist|neither persists|不存储/i,
      );
      expect(document.content, document.path).toMatch(
        /not.*upload|never.*upload|neither.*upload|不上传/i,
      );
    }
  });

  it("keeps every v0.9 planning and evidence document at Preview or Alpha with a physical No-Go gate", () => {
    const documents = readLocalLinkV09Documents();
    for (const document of documents) {
      expect(document.content, document.path).toMatch(/Preview|Alpha/);
      expect(document.content, document.path).toMatch(/No-Go/);
      expect(document.content, document.path).toMatch(
        /(two|双)[\s\S]{0,30}(real|physical|真机|Mac)|两台[\s\S]{0,30}(真实|Mac)/i,
      );
    }
    expect(documents.map(({ content }) => content).join("\n")).toMatch(/50[\s\S]{0,100}200/);
  });

  it("keeps v3 reconciliation and Copy recovery content-free and non-replayable", () => {
    const protocol = sourceFile("src-tauri/src/local_link/protocol.rs");
    const native = sourceFile("src-tauri/src/local_link.rs");
    const migrations = sourceFile("src-tauri/src/db/migrations.rs");
    const claimSchema = sourceSection(
      migrations,
      "CREATE TABLE local_link_effect_claims (",
      "CREATE INDEX idx_local_link_effect_claims_recovery",
    );
    const lifecycle = sourceFile("src-tauri/src/local_link/lifecycle_macos.rs");

    expect(native).toContain("const PROTOCOL_VERSION: u16 = 3");
    expect(protocol).toContain("StatusResponseState");
    expect(claimSchema).toMatch(/effect_token/);
    expect(claimSchema).not.toMatch(
      /body|preview|digest|endpoint|device_id|clipboard_item_id|history_item/i,
    );
    expect(lifecycle).toMatch(/NSWorkspaceWillSleepNotification/);
    expect(lifecycle).toMatch(/NSWorkspaceSessionDidResignActiveNotification/);
    expect(lifecycle).toMatch(/nw_path_monitor_create/);
    for (const document of readBoundaryDocuments()) {
      expect(document.content, document.path).toMatch(/outcomeUnknown/);
      expect(document.content, document.path).toMatch(
        /(never|no|not|禁止|不)[\s\S]{0,80}(replay|重放)/i,
      );
    }
  });
});
