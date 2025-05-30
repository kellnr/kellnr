#!/usr/bin/env lua

local testing = require("testing_lib")
local os = require("os")

-- Parse command line for our test script
-- Take image as first argument, but allow for flags anywhere
local image
local args = {}

for i, arg_value in ipairs(_G.arg) do
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
	container = "kellnr-sparse",
	registry = "kellnr-test",
	url = "http://localhost:8000",
	server_timeout = 60, -- seconds
	logs_dir = "logs"
}

-- Main function
local function main()
	-- Check image argument
	if not image then
		testing.log("Usage: " .. arg[0] .. " <image> [--debug/-d]", true)
		testing.log("Options:", true)
		testing.log("  --debug, -d    Enable debug logging", true)
		os.exit(1)
	end

	testing.log("Test sparse registry of Kellnr:" .. image, true)
	testing.debug_log("Debug mode is enabled", true)

	-- Create logs directory
	testing.create_directory(config.logs_dir)

	-- Start the container
	local env_vars = {
		["KELLNR_LOG__LEVEL"] = "debug",
		["KELLNR_LOG__LEVEL_WEB_SERVER"] = "debug",
		["KELLNR_PROXY__ENABLED"] = "true"
	}

	local ports = { ["8000"] = "8000" }

	local run_success = testing.docker_run(config.container, image, ports, env_vars)
	if not run_success then
		testing.error_log("Failed to start Docker container", true)
		os.exit(1)
	end

	-- Start logging in the background
	testing.docker_logs(config.container, "./logs/kellnr-sparse.log", true)

	-- Wait for server to start
	if not testing.wait_for_server(config.url, config.server_timeout) then
		testing.error_log("Server did not start properly", true)
		testing.docker_stop(config.container)
		os.exit(1)
	end

	local start_time = testing.start_timer()

	-- Publish crates
	local crates = {
		{ path = "crates/test_lib",          name = "test_lib" },
		{ path = "crates/UpperCase-Name123", name = "UpperCase-Name123" },
		{ path = "crates/foo-bar",           name = "foo-bar" }
	}

	for _, crate in ipairs(crates) do
		testing.log("Publishing crate: " .. crate.name, true)
		local publish_success = testing.publish_crate(crate.path, config.registry)
		if not publish_success then
			testing.error_log("Failed to publish crate: " .. crate.name, true)
			testing.docker_stop(config.container)
			os.exit(1)
		end
	end

	testing.end_timer(start_time, "Test execution time")

	-- Stop the container
	testing.log("Stopping Kellnr: " .. image, true)
	testing.docker_stop(config.container)

	testing.log("Done", true)
end

-- Run the program
main()
