fn main() {
    if cfg!(target_os = "windows") {
        winres::WindowsResource::new()
            .set_manifest(
                r#"
<assembly xmlns="urn:schemas-microsoft-com:asm.v1" manifestVersion="1.0">
    <assemblyIdentity
        version="1.0.0.0"
        processorArchitecture="*"
        name="VisualTrace"
        type="win32"
    />
    <description>Visual IP Trace Tool</description>
    <trustInfo xmlns="urn:schemas-microsoft-com:asm.v3">
        <security>
            <requestedPrivileges>
                <requestedExecutionLevel
                    level="requireAdministrator"
                    uiAccess="false"
                />
            </requestedPrivileges>
        </security>
    </trustInfo>
</assembly>
"#,
            )
            .compile()?;
    }
}
