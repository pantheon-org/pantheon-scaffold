// The build sets BUILD_VERSION (bun build --define), so a released binary reports the version it was built for. Running from
// source reports 0.0.0. The runner and the package are released together under one version.
declare const BUILD_VERSION: string | undefined;
export const VERSION: string = typeof BUILD_VERSION === "string" ? BUILD_VERSION : "0.0.0";
