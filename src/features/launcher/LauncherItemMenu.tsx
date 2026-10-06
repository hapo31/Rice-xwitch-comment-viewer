import * as DropdownMenu from "@radix-ui/react-dropdown-menu";
import { Ellipsis, Trash2 } from "lucide-react";
import { useEffect, useRef, useState } from "react";

/** Owns menu interaction; launching and removing applications stay in LauncherView. */
export function LauncherItemMenu({
  name,
  disabled,
  onRemove,
}: {
  name: string;
  disabled: boolean;
  onRemove: () => void;
}) {
  const [open, setOpen] = useState(false);
  const trigger = useRef<HTMLButtonElement>(null);
  const leavingWithTab = useRef(false);
  useEffect(() => {
    if (disabled) setOpen(false);
  }, [disabled]);
  return (
    <DropdownMenu.Root modal={false} open={open} onOpenChange={setOpen}>
      <DropdownMenu.Trigger asChild>
        <button
          ref={trigger}
          type="button"
          aria-label={`${name} のメニュー`}
          disabled={disabled}
          onPointerDown={(event) => event.stopPropagation()}
          onClick={(event) => event.stopPropagation()}
          className="absolute bottom-0 right-0 z-10 flex h-9 w-9 items-center justify-center text-white/80 hover:bg-black/25 hover:text-white disabled:opacity-50"
        >
          <Ellipsis className="h-4 w-4" aria-hidden="true" />
        </button>
      </DropdownMenu.Trigger>
      <DropdownMenu.Portal>
        <DropdownMenu.Content
          aria-label={`${name} の操作`}
          aria-labelledby={undefined}
          side="top"
          align="end"
          sideOffset={4}
          loop
          onPointerDown={(event) => event.stopPropagation()}
          onClick={(event) => event.stopPropagation()}
          onKeyDownCapture={(event) => {
            // Radix menus consume Tab. This nonmodal launcher menu must allow
            // native traversal from its trigger in either direction.
            if (event.key === "Tab") {
              event.stopPropagation();
              leavingWithTab.current = true;
              trigger.current?.focus();
              setOpen(false);
            }
          }}
          onCloseAutoFocus={(event) => {
            if (leavingWithTab.current) {
              event.preventDefault();
              leavingWithTab.current = false;
            }
          }}
          className="z-40 min-w-32 border border-zinc-700 bg-zinc-850 py-1 text-zinc-100 shadow-xl"
        >
          <DropdownMenu.Item
            disabled={disabled}
            onSelect={onRemove}
            className="flex cursor-default items-center gap-2 px-3 py-2 text-xs outline-none data-highlighted:bg-zinc-700 data-highlighted:text-rose-200 data-disabled:opacity-50"
          >
            <Trash2 className="h-3.5 w-3.5" aria-hidden="true" />
            削除
          </DropdownMenu.Item>
        </DropdownMenu.Content>
      </DropdownMenu.Portal>
    </DropdownMenu.Root>
  );
}
