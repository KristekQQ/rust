// Disabling Chromium's sandbox is an explicit opt-in for disposable Linux CI only.
export function browserArgs({ software = false, platform = process.platform, env = process.env } = {}) {
    const args = ['--enable-unsafe-webgpu'];
    if (software) args.push('--use-angle=swiftshader', '--enable-unsafe-swiftshader');
    const disposableCI = env.GITHUB_ACTIONS === 'true' && env.BROWSER_CI_NO_SANDBOX === '1';
    if (platform === 'linux' && disposableCI) args.push('--no-sandbox');
    return args;
}
