export const HOTKEYS = {
  SEARCH: 'ctrl+f',
  UPDATE_ALL: 'ctrl+u',
  TAB_INSTALLED: 'ctrl+1',
  TAB_FIND_MORE: 'ctrl+2',
  CLOSE_MODAL: 'escape'
} as const;

export type HotkeyAction = keyof typeof HOTKEYS;
