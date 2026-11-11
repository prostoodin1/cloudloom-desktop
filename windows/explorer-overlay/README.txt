Cloudloom Rowan: optional Explorer status icons (Windows x64)

1. Extract the complete ZIP to a folder.
2. Open 64-bit PowerShell as administrator in that folder.
3. Run .\register.ps1
4. Finish file transfers before signing out of Windows. Sign in again, open
   Cloudloom and connect the Explorer disk.

The blue icon shows a queued or running upload. A green check shows a recent
successful upload. The icons use status data from the running Cloudloom app.
Windows may limit how many overlay handlers it loads.

Remove this extension from an administrator PowerShell using
.\register.ps1 -Uninstall
