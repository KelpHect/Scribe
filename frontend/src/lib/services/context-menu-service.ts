export type ContextMenuSeparator = {
  type: 'separator';
};

export type ContextMenuItem = {
  type?: 'item';
  label: string;
  icon?: any;
  action: () => void | Promise<unknown>;
  disabled?: boolean;
  variant?: 'default' | 'destructive';
};

export type ContextMenuEntry = ContextMenuItem | ContextMenuSeparator;

export function isSeparator(entry: ContextMenuEntry): entry is ContextMenuSeparator {
  return 'type' in entry && entry.type === 'separator';
}

export function openContextMenuAt(x: number, y: number, items: ContextMenuEntry[]) {
  window.dispatchEvent(
    new CustomEvent('scribe:open-context-menu', {
      detail: {
        x,
        y,
        items
      }
    })
  );
}

export function openContextMenu(event: MouseEvent, items: ContextMenuEntry[]) {
  event.preventDefault();
  openContextMenuAt(event.clientX, event.clientY, items);
}
