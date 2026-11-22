import { contextBridge, ipcRenderer } from 'electron'

const api = {
  loadState: (): Promise<unknown> => ipcRenderer.invoke('state:load'),
  saveState: (state: unknown): Promise<void> => ipcRenderer.invoke('state:save', state),
  exportFile: (
    name: string,
    content: string
  ): Promise<{ canceled: boolean; path?: string }> =>
    ipcRenderer.invoke('file:export', name, content)
}

contextBridge.exposeInMainWorld('frog', api)

export type FrogApi = typeof api
