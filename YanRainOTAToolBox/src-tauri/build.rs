fn main() {
    // 使用 protox（纯 Rust protobuf 编译器）替代 protoc
    let file_descriptors = protox::compile(&["proto/update_metadata.proto"], &["proto/"])
        .expect("Failed to compile protobuf with protox");

    prost_build::compile_fds(file_descriptors).expect("Failed to generate Rust code from protobuf");

    // Windows: 自定义 manifest，要求以管理员权限运行
    // ADB/Fastboot/EDL/驱动安装等操作需要管理员权限
    let mut windows = tauri_build::WindowsAttributes::new();
    windows = windows.app_manifest(
        r#"
<assembly xmlns="urn:schemas-microsoft-com:asm.v1" manifestVersion="1.0">
  <dependency>
    <dependentAssembly>
      <assemblyIdentity
        type="win32"
        name="Microsoft.Windows.Common-Controls"
        version="6.0.0.0"
        processorArchitecture="*"
        publicKeyToken="6595b64144ccf1df"
        language="*"
      />
    </dependentAssembly>
  </dependency>
  <trustInfo xmlns="urn:schemas-microsoft-com:asm.v3">
    <security>
      <requestedPrivileges>
        <requestedExecutionLevel level="requireAdministrator" uiAccess="false" />
      </requestedPrivileges>
    </security>
  </trustInfo>
</assembly>
"#,
    );
    let attrs = tauri_build::Attributes::new().windows_attributes(windows);
    tauri_build::try_build(attrs).expect("failed to run tauri build script");
}
