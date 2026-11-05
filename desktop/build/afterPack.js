// electron-builder needs Wine to brand Noted.exe on Linux. resedit does the same in plain
// JavaScript: it sets the app icon and the version details shown in Explorer.
const path = require('path');
const fs = require('fs');

module.exports = async function afterPack(ctx) {
  if (ctx.electronPlatformName !== 'win32') return;
  const ResEdit = await import('resedit');
  const R = ResEdit.default || ResEdit;
  const exe = path.join(ctx.appOutDir, `${ctx.packager.appInfo.productFilename}.exe`);
  const ico = path.join(ctx.packager.info.buildResourcesDir, 'icon.ico');
  const version = ctx.packager.appInfo.version;
  const exeObj = R.NtExecutable.from(fs.readFileSync(exe), { ignoreCert: true });
  const rsrc = R.NtExecutableResource.from(exeObj);
  const iconFile = R.Data.IconFile.from(fs.readFileSync(ico));
  R.Resource.IconGroupEntry.replaceIconsForResource(rsrc.entries, 1, 1033, iconFile.icons.map(i => i.data));
  const vi = R.Resource.VersionInfo.fromEntries(rsrc.entries)[0];
  const [a, b, c] = version.split('.').map(Number);
  vi.setFileVersion(a, b, c, 0, 1033); vi.setProductVersion(a, b, c, 0, 1033);
  vi.setStringValues({ lang: 1033, codepage: 1200 }, {
    FileDescription: 'Noted', ProductName: 'Noted', CompanyName: 'Noted', OriginalFilename: 'Noted.exe', InternalName: 'Noted', LegalCopyright: 'Noted',
  });
  vi.outputToResourceEntries(rsrc.entries);
  rsrc.outputResource(exeObj);
  fs.writeFileSync(exe, Buffer.from(exeObj.generate()));
};
