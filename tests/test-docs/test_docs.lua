#!/usr/bin/env lua

local testing = require("testing_lib")
local os = require("os")
local socket = require("socket")
local lfs = require("lfs") -- LuaFileSystem, used for directory operations

-- Parse command line for our test script
-- Take image as first argument, but allow for flags anywhere
local image
local args = {}
for i, arg_value in ipairs(arg) do
    if arg_value:sub(1, 1) == "-" then
        -- This is a flag, keep it in args table
        table.insert(args, arg_value)
    else
        -- This is likely the image
        image = arg_value
    end
end

-- Configuration
local config = {
    container = "kellnr-docs",
    registry = "kellnr-test",
    url = "http://localhost:8000",
    server_timeout = 60, -- seconds
    logs_dir = "logs",
    data_dir = "./data",
    docs_retry_attempts = 30,
    docs_check_delay = 2, -- seconds
}

-- Helper function to check if a file exists
local function file_exists(path)
    local file = io.open(path, "r")
    if file then
        file:close()
        return true
    end
    return false
end

-- Helper function to remove directory recursively
local function remove_directory(path)
    if lfs.attributes(path, "mode") ~= "directory" then
        return true
    end

    testing.log("Removing directory: " .. path, true)
    return testing.exec_with_logging("rm -rf " .. path, false)
end

-- Main function
local function main()
    -- Check image argument
    if not image then
        testing.log("Usage: " .. arg[0] .. " <image> [--debug/-d]", true)
        testing.log("Options:", true)
        testing.log("  --debug, -d    Enable debug logging", true)
        os.exit(1)
    end

    testing.log("Test docs registry of Kellnr:" .. image, true)
    testing.debug_log("Debug mode is enabled", true)

    -- Create logs directory
    testing.create_directory(config.logs_dir)

    -- Remove data directory if it exists
    if lfs.attributes(config.data_dir, "mode") == "directory" then
        testing.log("Removing existing data directory...", true)
        if not remove_directory(config.data_dir) then
            testing.error_log("Failed to remove data directory", true)
            os.exit(1)
        end
    end

    -- Create data directory
    if not testing.create_directory(config.data_dir) then
        testing.error_log("Failed to create data directory", true)
        os.exit(1)
    end

    -- Get absolute path for volume mounting
    local current_dir = lfs.currentdir()
    local data_path = current_dir .. "/" .. config.data_dir:gsub("^%./", "")

    -- Start Kellnr container
    testing.log("Starting Kellnr:" .. image, true)
    local env_vars = {
        ["KELLNR_LOG__LEVEL"] = "trace",
        ["KELLNR_LOG__LEVEL_WEB_SERVER"] = "debug",
        ["KELLNR_DOCS__ENABLED"] = "true"
    }

    local ports = { ["8000"] = "8000" }
    local additional_params = "-v " .. data_path .. ":/opt/kdata/"

    local run_success = testing.docker_run(config.container, image, ports, env_vars, additional_params)
    if not run_success then
        testing.error_log("Failed to start Docker container", true)
        os.exit(1)
    end

    -- Start logging
    testing.docker_logs(config.container, "./logs/kellnr-docs.log", true)

    -- Wait for server to start
    if not testing.wait_for_server(config.url, config.server_timeout) then
        testing.error_log("Server did not start properly", true)
        testing.docker_stop(config.container)
        os.exit(1)
    end

    local start_time = testing.start_timer()

    -- Publish the full-toml crate
    testing.log("Publishing full-toml crate", true)
    local publish_success = testing.publish_crate("crates/full-toml", config.registry)
    if not publish_success then
        testing.error_log("Failed to publish full-toml crate", true)
        testing.docker_stop(config.container)
        os.exit(1)
    end

    -- Check for documentation generation
    testing.log("Checking if docs were generated...", true)
    local docs_path = config.data_dir .. "/docs/full-toml/1.0.0/doc/full_toml/index.html"
    local docs_generated = false

    for i = 1, config.docs_retry_attempts do
        testing.debug_log("Checking for docs attempt " .. i .. "/" .. config.docs_retry_attempts, true)
        if file_exists(docs_path) then
            docs_generated = true
            break
        end
        socket.sleep(config.docs_check_delay)
    end

    testing.end_timer(start_time, "Test execution time")

    -- Stop the container
    testing.log("Stopping Kellnr: " .. image, true)
    testing.docker_stop(config.container)

    if docs_generated then
        testing.log("Docs generated successfully.", true)
        testing.log("Done", true)
        return true
    else
        testing.error_log("Docs not generated successfully.", true)
        os.exit(1)
    end
end

-- Run the program
main()
