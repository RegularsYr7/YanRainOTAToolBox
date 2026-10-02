; ═══════════════════════════════════════════════════════
; 烟雨OTA工具箱 - NSIS 安装器钩子脚本
; ═══════════════════════════════════════════════════════
;
; 此文件定义安装/卸载过程中的自定义行为，
; 由 Tauri 的 NSIS 打包器自动调用。

; ───────────────────────────────────────────────────────
; 桌面快捷方式重命名支持
; ───────────────────────────────────────────────────────
; 交互式安装时，桌面快捷方式由完成页复选框回调创建，
; 该回调晚于 POSTINSTALL 钩子执行，因此借助 .onGUIEnd
; 在 GUI 关闭前完成重命名。
Var _DesktopLnkOld
Var _DesktopLnkNew

Function .onGUIEnd
  ${If} ${FileExists} "$_DesktopLnkOld"
    Rename "$_DesktopLnkOld" "$_DesktopLnkNew"
  ${EndIf}
  ; 通知 Shell 刷新桌面图标（SHCNE_ASSOCCHANGED）
  System::Call 'shell32::SHChangeNotify(i 0x08000000, i 0, p 0, p 0)'
FunctionEnd

; ───────────────────────────────────────────────────────
; 安装前钩子
; 在复制文件、设置注册表、创建快捷方式之前执行
; ───────────────────────────────────────────────────────
!macro NSIS_HOOK_PREINSTALL
  ; === 0. 禁止安装到包含非 ASCII 字符（中文等）的路径 ===
  System::Call 'kernel32::WideCharToMultiByte(i 20127, i 0, w "$INSTDIR", i -1, i 0, i 0, i 0, *i 0 r1) i'
  ${If} $1 != 0
    ; 取当前路径的盘符（如 C: 或 D:），在同盘根目录下推荐安装
    StrCpy $2 $INSTDIR 2
    MessageBox MB_YESNO|MB_ICONSTOP \
      "❌ 安装路径不可用$\n$\n\
当前安装路径包含中文或特殊字符：$\n\
$INSTDIR$\n$\n\
该路径会导致刷机、Boot修补、EDL深刷等核心功能异常，无法正常使用。$\n$\n\
点击「是」自动安装到推荐路径 $2\YanRainOTAToolBox$\n\
点击「否」退出安装，手动选择其他纯英文路径。" \
      IDYES _path_fix IDNO _path_abort
    _path_abort:
      Abort
    _path_fix:
      StrCpy $INSTDIR "$2\YanRainOTAToolBox"
      ; 重新设定输出目录，覆盖 Tauri 模板中已执行的 SetOutPath
      SetOutPath $INSTDIR
  ${EndIf}
  ; === 1. 检查并关闭主应用 ===
  FindWindow $0 "" "${PRODUCTNAME}"
  ${If} $0 != 0
    MessageBox MB_OKCANCEL|MB_ICONEXCLAMATION \
      "检测到 ${PRODUCTNAME} 正在运行，安装程序需要关闭它才能继续。$\n$\n点击「确定」关闭应用并继续安装，或点击「取消」退出安装。" \
      IDOK close_app IDCANCEL abort_install
    close_app:
      ; 尝试优雅关闭
      SendMessage $0 ${WM_CLOSE} 0 0
      Sleep 2000
      Goto kill_resources
    abort_install:
      Abort
  ${EndIf}

  ; 按进程名兜底：即使窗口标题不匹配，也确保主进程退出
  nsExec::ExecToLog 'cmd /c taskkill /F /IM "${PRODUCTNAME}.exe" 2>nul'
  Sleep 500

  kill_resources:
  ; === 2. 终止 resources 目录下可能被锁定的进程 ===
  ; adb.exe / fastboot.exe / scrcpy.exe / aria2c.exe / adb server 等
  ; 如果进程不存在 taskkill 会静默失败，不影响安装流程
  DetailPrint "正在释放工具资源文件..."

  ; 先优雅关闭 adb server（避免文件锁定）
  nsExec::ExecToLog 'cmd /c "$INSTDIR\resources\platform-tools\windows\adb.exe" kill-server 2>nul'
  ; 等待 adb server 退出
  Sleep 1000

  ; 强制终止可能残留的进程
  nsExec::ExecToLog 'cmd /c taskkill /F /IM adb.exe 2>nul'
  nsExec::ExecToLog 'cmd /c taskkill /F /IM fastboot.exe 2>nul'
  nsExec::ExecToLog 'cmd /c taskkill /F /IM scrcpy.exe 2>nul'
  nsExec::ExecToLog 'cmd /c taskkill /F /IM scrcpy-server 2>nul'
  nsExec::ExecToLog 'cmd /c taskkill /F /IM aria2c.exe 2>nul'
  nsExec::ExecToLog 'cmd /c taskkill /F /IM magiskboot.exe 2>nul'
  nsExec::ExecToLog 'cmd /c taskkill /F /IM ksud.exe 2>nul'
  nsExec::ExecToLog 'cmd /c taskkill /F /IM fh_loader.exe 2>nul'
  nsExec::ExecToLog 'cmd /c taskkill /F /IM QSaharaServer.exe 2>nul'

  ; 等待进程完全退出，确保文件句柄释放
  Sleep 2000
  DetailPrint "资源文件已释放"
!macroend

; ───────────────────────────────────────────────────────
; 安装后钩子
; 在所有文件复制完成、注册表设置完毕、快捷方式创建后执行
; ───────────────────────────────────────────────────────
!macro NSIS_HOOK_POSTINSTALL
  ; 设置变量供 .onGUIEnd 使用（交互式安装时桌面快捷方式在完成页才创建）
  StrCpy $_DesktopLnkOld "$DESKTOP\${PRODUCTNAME}.lnk"
  StrCpy $_DesktopLnkNew "$DESKTOP\烟雨OTA工具箱v${VERSION}.lnk"

  ; 升级安装时：清理旧版本的重命名快捷方式
  StrCpy $R0 "0"
  FindFirst $0 $1 "$DESKTOP\烟雨OTA工具箱v*.lnk"
  ${If} $1 != ""
    StrCpy $R0 "1"
  ${EndIf}
  FindClose $0
  ${If} $R0 == "1"
    Delete "$DESKTOP\烟雨OTA工具箱v*.lnk"
  ${EndIf}

  ; 重命名桌面快捷方式
  ${If} ${FileExists} "$DESKTOP\${PRODUCTNAME}.lnk"
    ; 静默/被动安装：快捷方式已创建，直接重命名
    Rename "$DESKTOP\${PRODUCTNAME}.lnk" "$DESKTOP\烟雨OTA工具箱v${VERSION}.lnk"
  ${ElseIf} $R0 == "1"
    ; 升级安装：旧版快捷方式已删除，创建新版
    CreateShortcut "$DESKTOP\烟雨OTA工具箱v${VERSION}.lnk" "$INSTDIR\${MAINBINARYNAME}.exe"
    !insertmacro SetLnkAppUserModelId "$DESKTOP\烟雨OTA工具箱v${VERSION}.lnk"
  ${EndIf}

  ; 重命名开始菜单快捷方式
  ${If} ${FileExists} "$SMPROGRAMS\RainyOTAToolBox\${PRODUCTNAME}.lnk"
    Rename "$SMPROGRAMS\RainyOTAToolBox\${PRODUCTNAME}.lnk" "$SMPROGRAMS\RainyOTAToolBox\烟雨OTA工具箱v${VERSION}.lnk"
  ${Else}
    ; 升级安装：清理旧版，创建新版
    StrCpy $R0 "0"
    FindFirst $0 $1 "$SMPROGRAMS\RainyOTAToolBox\烟雨OTA工具箱v*.lnk"
    ${If} $1 != ""
      StrCpy $R0 "1"
    ${EndIf}
    FindClose $0
    ${If} $R0 == "1"
      Delete "$SMPROGRAMS\RainyOTAToolBox\烟雨OTA工具箱v*.lnk"
      CreateShortcut "$SMPROGRAMS\RainyOTAToolBox\烟雨OTA工具箱v${VERSION}.lnk" "$INSTDIR\${MAINBINARYNAME}.exe"
      !insertmacro SetLnkAppUserModelId "$SMPROGRAMS\RainyOTAToolBox\烟雨OTA工具箱v${VERSION}.lnk"
    ${EndIf}
  ${EndIf}

  ; 写入额外的注册表信息（应用描述）
  WriteRegStr SHCTX "Software\Microsoft\Windows\CurrentVersion\Uninstall\${PRODUCTNAME}" \
    "URLInfoAbout" "https://github.com/RegularsYr7/YanRainToolBox-Tauri-Rust-React"
  WriteRegStr SHCTX "Software\Microsoft\Windows\CurrentVersion\Uninstall\${PRODUCTNAME}" \
    "HelpLink" "https://github.com/RegularsYr7/YanRainToolBox-Tauri-Rust-React/issues"
  WriteRegStr SHCTX "Software\Microsoft\Windows\CurrentVersion\Uninstall\${PRODUCTNAME}" \
    "Publisher" "酷安@烟雨ovo"
  WriteRegStr SHCTX "Software\Microsoft\Windows\CurrentVersion\Uninstall\${PRODUCTNAME}" \
    "Comments" "现代化 Android 玩机工具箱"

  ; 通知 Shell 刷新桌面图标（SHCNE_ASSOCCHANGED）
  System::Call 'shell32::SHChangeNotify(i 0x08000000, i 0, p 0, p 0)'
!macroend

; ───────────────────────────────────────────────────────
; 卸载前钩子
; 在删除文件、注册表、快捷方式之前执行
; ───────────────────────────────────────────────────────
!macro NSIS_HOOK_PREUNINSTALL
  ; 确保应用已关闭
  FindWindow $0 "" "${PRODUCTNAME}"
  ${If} $0 != 0
    MessageBox MB_OKCANCEL|MB_ICONEXCLAMATION \
      "检测到 ${PRODUCTNAME} 正在运行，卸载程序需要关闭它才能继续。$\n$\n点击「确定」关闭应用并继续卸载，或点击「取消」退出。" \
      IDOK close_uninstall IDCANCEL abort_uninstall
    close_uninstall:
      SendMessage $0 ${WM_CLOSE} 0 0
      Sleep 2000
      Goto kill_resources_uninstall
    abort_uninstall:
      Abort
  ${EndIf}

  ; 按进程名兜底
  nsExec::ExecToLog 'cmd /c taskkill /F /IM "${PRODUCTNAME}.exe" 2>nul'
  Sleep 500

  kill_resources_uninstall:
  ; 终止 resources 目录下可能被锁定的进程
  DetailPrint "正在释放工具资源文件..."
  nsExec::ExecToLog 'cmd /c "$INSTDIR\resources\platform-tools\windows\adb.exe" kill-server 2>nul'
  Sleep 1000
  nsExec::ExecToLog 'cmd /c taskkill /F /IM adb.exe 2>nul'
  nsExec::ExecToLog 'cmd /c taskkill /F /IM fastboot.exe 2>nul'
  nsExec::ExecToLog 'cmd /c taskkill /F /IM scrcpy.exe 2>nul'
  nsExec::ExecToLog 'cmd /c taskkill /F /IM scrcpy-server 2>nul'
  nsExec::ExecToLog 'cmd /c taskkill /F /IM aria2c.exe 2>nul'
  nsExec::ExecToLog 'cmd /c taskkill /F /IM magiskboot.exe 2>nul'
  nsExec::ExecToLog 'cmd /c taskkill /F /IM ksud.exe 2>nul'
  nsExec::ExecToLog 'cmd /c taskkill /F /IM fh_loader.exe 2>nul'
  nsExec::ExecToLog 'cmd /c taskkill /F /IM QSaharaServer.exe 2>nul'
  Sleep 2000
  DetailPrint "资源文件已释放"

  ; 删除重命名后的桌面和开始菜单快捷方式（默认卸载器只删除原名快捷方式）
  Delete "$DESKTOP\烟雨OTA工具箱v*.lnk"
  Delete "$SMPROGRAMS\RainyOTAToolBox\烟雨OTA工具箱v*.lnk"
!macroend

; ───────────────────────────────────────────────────────
; 卸载后钩子
; 在文件、注册表、快捷方式全部删除后执行
; ───────────────────────────────────────────────────────
!macro NSIS_HOOK_POSTUNINSTALL
  ; 清理应用数据目录（可选）
  MessageBox MB_YESNO|MB_ICONQUESTION \
    "是否删除应用配置和数据？$\n$\n选择「是」将清除所有应用数据，选择「否」保留配置数据以便将来重新安装时使用。" \
    IDYES clean_data IDNO skip_clean
  clean_data:
    RMDir /r "$APPDATA\com.administrator.YanRainOTAToolBox"
    RMDir /r "$LOCALAPPDATA\com.administrator.YanRainOTAToolBox"
    ; 清理安装目录下运行时产生的残留文件（logs、root_apk 等）
    RMDir /r "$INSTDIR"
  skip_clean:
!macroend
