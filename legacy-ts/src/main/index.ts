import { app, BrowserWindow, dialog, ipcMain, shell } from 'electron'
import { mkdir, readFile, rename, writeFile } from 'node:fs/promises'
import { dirname, join } from 'node:path'

let mainWindow: BrowserWindow | null = null

function statePath(): string {
  return join(app.getPath('userData'), 'frogcalc-state.json')
}

async function loadState(): Promise<unknown> {
  try {
    const text = await readFile(statePath(), 'utf8')
    return JSON.parse(text)
  } catch {
    return null
  }
}

async function saveState(state: unknown): Promise<void> {
  const path = statePath()
  await mkdir(dirname(path), { recursive: true })
  const temp = `${path}.tmp`
  await writeFile(temp, JSON.stringify(state), 'utf8')
  await rename(temp, path)
}

function createWindow(): void {
  mainWindow = new BrowserWindow({
    width: 1320,
    height: 860,
    minWidth: 980,
    minHeight: 640,
    show: false,
    autoHideMenuBar: true,
    backgroundColor: '#0a0d13',
    title: 'FrogCalc',
    titleBarStyle: 'hidden',
    titleBarOverlay: {
      color: '#0a0d13',
      symbolColor: '#c9d4e3',
      height: 48
    },
    webPreferences: {
      preload: join(__dirname, '../preload/index.js'),
      contextIsolation: true,
      sandbox: true,
      nodeIntegration: false
    }
  })

  mainWindow.on('ready-to-show', () => {
    mainWindow?.show()
  })

  mainWindow.webContents.setWindowOpenHandler(({ url }) => {
    void shell.openExternal(url)
    return { action: 'deny' }
  })

  const devUrl = process.env['ELECTRON_RENDERER_URL']
  if (devUrl) {
    void mainWindow.loadURL(devUrl)
  } else {
    void mainWindow.loadFile(join(__dirname, '../renderer/index.html'))
  }

  mainWindow.on('closed', () => {
    mainWindow = null
  })
}

app.whenReady().then(() => {
  app.setAppUserModelId('com.frogcalc.app')

  ipcMain.handle('state:load', () => loadState())
  ipcMain.handle('state:save', (_event, state: unknown) => saveState(state))
  ipcMain.handle('file:export', async (_event, name: string, content: string) => {
    if (!mainWindow) return { canceled: true }
    const result = await dialog.showSaveDialog(mainWindow, {
      title: '导出',
      defaultPath: name,
      filters: [
        { name: '文本文件', extensions: ['txt', 'md'] },
        { name: '所有文件', extensions: ['*'] }
      ]
    })
    if (result.canceled || !result.filePath) return { canceled: true }
    await writeFile(result.filePath, content, 'utf8')
    return { canceled: false, path: result.filePath }
  })

  createWindow()

  app.on('activate', () => {
    if (BrowserWindow.getAllWindows().length === 0) createWindow()
  })
})

app.on('window-all-closed', () => {
  if (process.platform !== 'darwin') app.quit()
})
