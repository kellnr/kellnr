#!/usr/bin/env lua

local testing = require("testing_lib")
local os = require("os")
local http_request = require('http.request')
local json = require('cjson')

-- Configuration for test scripts and directories
local test_scripts = {
    { dir = "./test-migration",       script = "test_migrations.lua", description = "MIGRATION TEST",               requires_latest_version = true },
    { dir = "./test-sparse-registry", script = "test_sparse_reg.lua", description = "SPARSE REGISTRY TEST",         requires_latest_version = false },
    { dir = "./test-auth-req",        script = "test_auth_req.lua",   description = "REGISTRY AUTHENTICATION TEST", requires_latest_version = false },
    { dir = "./test-s3-storage",      script = "test_s3_storage.lua", description = "S3 STORAGE TEST",              requires_latest_version = false },
    { dir = "./test-docs",            script = "test_docs.lua",       description = "DOCS GENERATION TEST",         requires_latest_version = false }
}

-- Helper function to run a test script
local function run_test(dir, script, new_version, latest_version, description, requires_latest_version)
    testing.log("--- RUN " .. description .. " ---", true)
    local cmd
    if requires_latest_version then
        cmd = string.format("cd %s && lua %s %s %s", dir, script, latest_version, new_version)
    else
        cmd = string.format("cd %s && lua %s %s", dir, script, new_version)
    end
    local success, _, exit_code = os.execute(cmd)
    if not success or exit_code ~= 0 then
        testing.error_log("Test failed: " .. description, true)
        os.exit(1)
    end
end

-- Function to fetch releases from GitHub API
local function fetch_releases()
    local request = http_request.new_from_uri("https://api.github.com/repos/kellnr/kellnr/releases")
    request.headers:upsert(":method", "GET")
    request.headers:upsert("user-agent", "kellnr-test-script/1.0")

    local headers, stream = request:go()
    if not headers then
        return nil
    end

    local body = stream:get_body_as_string()
    return body
end

-- Function to find latest release with more than min_assets
local function find_latest_with_assets(releases, min_assets)
    min_assets = min_assets or 2

    for _, release in ipairs(releases) do
        if release.assets and #release.assets > min_assets then
            return string.sub(release.tag_name, 2) -- Remove the 'v' prefix
        end
    end

    return nil
end

-- Build a testing image for Kellnr
local function build_kellnr(image)
    testing.log("--- BUILDING KELLNR TESTING IMAGE ---", true)
    testing.docker_build(image, { KELLNR_VERSION = "local" }, "../")
    testing.log("Kellnr testing image built successfully: " .. image, true)
end


-- Main function
local function main()
    local image = "kellnr-test:local"

    -- Fetch the latest version of Kellnr from GitHub
    local json_data = fetch_releases()
    if not json_data or json_data == "" then
        testing.error_log("Failed to fetch release data from GitHub API.", true)
        os.exit(1)
    end

    local success, releases = pcall(json.decode, json_data)
    if not success then
        testing.error_log("Failed to parse JSON response from GitHub API.", true)
        os.exit(1)
    end

    local latest_version = find_latest_with_assets(releases, 2)
    if not latest_version then
        testing.error_log("No release found with more than 2 assets.", true)
        os.exit(1)
    end

    local latest_image = "ghcr.io/kellnr/kellnr:" .. latest_version

    testing.log("INFO: Latest released version of Kellnr is: " .. latest_version, true)

    -- Build the Kellnr testing image
    build_kellnr(image)

    -- Run each test script
    for _, test in ipairs(test_scripts) do
        run_test(test.dir, test.script, image, latest_image, test.description, test.requires_latest_version)
    end

    testing.log("--- ALL TESTS FINISHED ---", true)
end

-- Run the script
main()
