import {
  BookmarkMinus,
  BookmarkPlus,
  ClipboardPaste,
  Copy,
  ListPlus,
  MoreHorizontal,
  Send,
  Tags,
  TextCursorInput,
  Trash2,
} from "lucide-react";
import {
  type KeyboardEvent as ReactKeyboardEvent,
  useEffect,
  useLayoutEffect,
  useMemo,
  useRef,
  useState,
} from "react";
import "./ClipboardActionPanel.css";

interface ClipboardActionPanelProps {
  isSaved: boolean;
  onPaste?: () => unknown;
  onPlainTextPaste?: () => unknown;
  onCopy?: () => unknown;
  onToggleSaved?: () => unknown;
  onManageCollections?: () => void;
  onEnqueue?: () => void;
  onSend?: () => void;
  onRecycle?: () => unknown;
  shortcutEnabled?: boolean;
  triggerLabel?: string;
  triggerClassName?: string;
}

interface ActionDefinition {
  id: string;
  section: "activate" | "organize" | "share" | "danger";
  label: string;
  shortcut?: string;
  icon: typeof Copy;
  danger?: boolean;
  run: () => unknown;
}

export function ClipboardActionPanel({
  isSaved,
  onPaste,
  onPlainTextPaste,
  onCopy,
  onToggleSaved,
  onManageCollections,
  onEnqueue,
  onSend,
  onRecycle,
  shortcutEnabled = false,
  triggerLabel = "More item actions",
  triggerClassName = "icon-button",
}: ClipboardActionPanelProps) {
  const [open, setOpen] = useState(false);
  const [confirmingRecycle, setConfirmingRecycle] = useState(false);
  const [activeIndex, setActiveIndex] = useState(0);
  const triggerRef = useRef<HTMLButtonElement>(null);
  const menuRef = useRef<HTMLDivElement>(null);
  const actionRefs = useRef<Array<HTMLButtonElement | null>>([]);
  const cancelRecycleRef = useRef<HTMLButtonElement>(null);
  const wasOpenRef = useRef(false);

  const actions = useMemo(() => {
    const definitions: ActionDefinition[] = [];
    if (onPaste) {
      definitions.push({
        id: "paste",
        section: "activate",
        label: shortcutEnabled ? "Direct Paste" : "Paste",
        shortcut: shortcutEnabled ? undefined : "↵",
        icon: ClipboardPaste,
        run: onPaste,
      });
    }
    if (onPlainTextPaste) {
      definitions.push({
        id: "plain-text",
        section: "activate",
        label: "Paste as plain text",
        shortcut: shortcutEnabled ? undefined : "⇧↵",
        icon: TextCursorInput,
        run: onPlainTextPaste,
      });
    }
    if (onCopy) {
      definitions.push({
        id: "copy",
        section: "activate",
        label: "Copy",
        shortcut: "⌘C",
        icon: Copy,
        run: onCopy,
      });
    }
    if (onToggleSaved) {
      definitions.push({
        id: "toggle-saved",
        section: "organize",
        label: isSaved ? "Remove from Saved" : "Save for reuse",
        icon: isSaved ? BookmarkMinus : BookmarkPlus,
        run: onToggleSaved,
      });
    }
    if (onManageCollections) {
      definitions.push({
        id: "manage-collections",
        section: "organize",
        label: "Manage Collections",
        icon: Tags,
        run: onManageCollections,
      });
    }
    if (onEnqueue) {
      definitions.push({
        id: "enqueue",
        section: "organize",
        label: "Add to Stack",
        icon: ListPlus,
        run: onEnqueue,
      });
    }
    if (onSend) {
      definitions.push({
        id: "send",
        section: "share",
        label: "Send to device",
        icon: Send,
        run: onSend,
      });
    }
    if (onRecycle) {
      definitions.push({
        id: "recycle",
        section: "danger",
        label: "Move to Recycle Bin",
        icon: Trash2,
        danger: true,
        run: () => setConfirmingRecycle(true),
      });
    }
    return definitions;
  }, [
    isSaved,
    onCopy,
    onManageCollections,
    onEnqueue,
    onPaste,
    onPlainTextPaste,
    onRecycle,
    onSend,
    onToggleSaved,
    shortcutEnabled,
  ]);

  const close = (restoreFocus = true) => {
    setOpen(false);
    setConfirmingRecycle(false);
    if (restoreFocus) triggerRef.current?.focus();
  };

  const openMenu = () => {
    setConfirmingRecycle(false);
    setActiveIndex(0);
    setOpen(true);
  };

  useLayoutEffect(() => {
    if (open) {
      wasOpenRef.current = true;
      actionRefs.current[0]?.focus();
      return;
    }
    if (wasOpenRef.current && document.activeElement === document.body) triggerRef.current?.focus();
  }, [open]);

  useLayoutEffect(() => {
    if (!confirmingRecycle) return;
    cancelRecycleRef.current?.focus();
  }, [confirmingRecycle]);

  useEffect(() => {
    if (!shortcutEnabled) return;
    const handleShortcut = (event: KeyboardEvent) => {
      if (!(event.metaKey || event.ctrlKey) || event.key.toLocaleLowerCase() !== "k") return;
      event.preventDefault();
      if (!open) {
        setConfirmingRecycle(false);
        setActiveIndex(0);
        setOpen(true);
      }
    };
    window.addEventListener("keydown", handleShortcut);
    return () => window.removeEventListener("keydown", handleShortcut);
  }, [open, shortcutEnabled]);

  const runAction = (action: ActionDefinition) => {
    if (action.id === "recycle") {
      action.run();
      return;
    }
    close();
    void action.run();
  };

  const moveFocus = (nextIndex: number) => {
    const bounded = (nextIndex + actions.length) % actions.length;
    setActiveIndex(bounded);
    actionRefs.current[bounded]?.focus();
  };

  const handleMenuKeyDown = (event: ReactKeyboardEvent<HTMLDivElement>) => {
    event.stopPropagation();
    if (event.key === "Escape") {
      event.preventDefault();
      if (confirmingRecycle) {
        setConfirmingRecycle(false);
        const recycleIndex = actions.findIndex((action) => action.id === "recycle");
        actionRefs.current[recycleIndex]?.focus();
      } else {
        close();
      }
      return;
    }
    if (confirmingRecycle || actions.length === 0) return;
    if (event.key === "ArrowDown") {
      event.preventDefault();
      moveFocus(activeIndex + 1);
    } else if (event.key === "ArrowUp") {
      event.preventDefault();
      moveFocus(activeIndex - 1);
    } else if (event.key === "Home") {
      event.preventDefault();
      moveFocus(0);
    } else if (event.key === "End") {
      event.preventDefault();
      moveFocus(actions.length - 1);
    }
  };

  const sections = ["activate", "organize", "share", "danger"] as const;

  return (
    <div className="clipboard-action-panel">
      <button
        ref={triggerRef}
        type="button"
        className={triggerClassName}
        aria-label={triggerLabel}
        aria-haspopup="menu"
        aria-expanded={open}
        aria-keyshortcuts={shortcutEnabled ? "Meta+K Control+K" : undefined}
        title="More actions"
        onClick={(event) => {
          event.stopPropagation();
          if (open) close();
          else openMenu();
        }}
      >
        <MoreHorizontal size={15} />
      </button>
      {open ? (
        <div
          ref={menuRef}
          className="clipboard-action-menu"
          role="menu"
          aria-label="Clipboard actions"
          onKeyDown={handleMenuKeyDown}
        >
          {confirmingRecycle ? (
            <section
              className="clipboard-action-confirmation"
              role="alertdialog"
              aria-label="Move clip to Recycle Bin?"
            >
              <strong>Move this clip to Recycle Bin?</strong>
              <p>You can restore it from Recycle Bin until its recovery period ends.</p>
              <div>
                <button
                  ref={cancelRecycleRef}
                  type="button"
                  onClick={() => setConfirmingRecycle(false)}
                >
                  Cancel
                </button>
                <button
                  type="button"
                  className="clipboard-action-confirm"
                  onClick={() => {
                    close();
                    void onRecycle?.();
                  }}
                >
                  Move to Recycle Bin
                </button>
              </div>
            </section>
          ) : (
            sections.map((section) => {
              const sectionActions = actions.filter((action) => action.section === section);
              if (sectionActions.length === 0) return null;
              return (
                <div key={section} className="clipboard-action-menu-section">
                  {sectionActions.map((action) => {
                    const index = actions.indexOf(action);
                    const Icon = action.icon;
                    return (
                      <button
                        key={action.id}
                        ref={(element) => {
                          actionRefs.current[index] = element;
                        }}
                        type="button"
                        role="menuitem"
                        tabIndex={activeIndex === index ? 0 : -1}
                        data-active={activeIndex === index}
                        className={action.danger ? "clipboard-action-danger" : undefined}
                        onFocus={() => setActiveIndex(index)}
                        onClick={(event) => {
                          event.stopPropagation();
                          runAction(action);
                        }}
                      >
                        <Icon size={15} aria-hidden="true" />
                        <span>{action.label}</span>
                        {action.shortcut ? <kbd>{action.shortcut}</kbd> : null}
                      </button>
                    );
                  })}
                </div>
              );
            })
          )}
        </div>
      ) : null}
    </div>
  );
}
