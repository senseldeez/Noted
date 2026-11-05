const { contextBridge, ipcRenderer } = require('electron');
const call = ch => (...a) => ipcRenderer.invoke(ch, ...a);
contextBridge.exposeInMainWorld('noted', {
  platform: process.platform,
  root: call('root'), scan: call('scan'), chooseRoot: call('chooseRoot'),
  write: call('write'), writeSync: (p, t) => ipcRenderer.sendSync('writeSync', p, t),
  createNote: call('createNote'), createFolder: call('createFolder'), rename: call('rename'),
  move: call('move'), trash: call('trash'), setPinned: call('setPinned'), saveOrder: call('saveOrder'),
  import: call('import'), reveal: call('reveal'),
  onFocus: fn => ipcRenderer.on('focus', fn),
});
