#!/usr/bin/env lua

local testing = require("testing_lib")
local os = require("os")
local lfs = require("lfs") -- LuaFileSystem for directory operations
local socket = require("socket")

-- Parse command line arguments
local old_image, new_image
local args = {}

for i, arg_value in ipairs(_G.arg) do
	if arg_value:sub(1, 1) == "-" then
		-- This is a flag, keep it in args table
		table.insert(args, arg_value)
	elseif not old_image then
		-- First non-flag argument is old_image
		old_image = arg_value
	elseif not new_image then
		-- Second non-flag argument is new_image
		new_image = arg_value
	end
end

-- Configuration
local config = {
	old_container = "kellnr-old",
	new_container = "kellnr-new",
	registry = "kellnr-local",
	url = "http://localhost:8000",
	server_timeout = 60, -- seconds
	kdata_dir = "kdata",
	logs_dir = "logs"
}

-- Helper function to remove directory
local function remove_directory(path)
	if lfs.attributes(path, "mode") ~= "directory" then
		return true
	end

	testing.log("Removing directory: " .. path, true)
	return testing.exec_with_logging("rm -rf " .. path, false)
end

-- Main function
local function main()
	-- Check version arguments
	if not old_image or not new_image then
		testing.log("Usage: " .. arg[0] .. " <old-image> <new-image> [--debug/-d]", true)
		testing.log("Options:", true)
		testing.log("  --debug, -d    Enable debug logging", true)
		os.exit(1)
	end

	testing.log("Test Kellnr:" .. old_image .. " -> Kellnr:" .. new_image, true)
	testing.debug_log("Debug mode is enabled", true)

	-- Pull Docker images
	testing.docker_pull(old_image)

	-- Remove and recreate directories
	remove_directory(config.kdata_dir)
	remove_directory(config.logs_dir)
	testing.create_directory(config.kdata_dir)
	testing.create_directory(config.logs_dir)

	-- Get absolute path for volume mounting
	local current_dir = lfs.currentdir()
	local kdata_path = current_dir .. "/" .. config.kdata_dir

	-- Start the old version container
	testing.log("Starting Kellnr:" .. old_image, true)
	local env_vars = {
		["KELLNR_LOG__LEVEL"] = "debug",
		["KELLNR_LOG__LEVEL_WEB_SERVER"] = "debug"
	}

	local ports = { ["8000"] = "8000" }
	local additional_params = "-v " .. kdata_path .. ":/opt/kdata"

	local run_success = testing.docker_run(config.old_container, old_image, ports, env_vars, additional_params)
	if not run_success then
		testing.error_log("Failed to start old Docker container", true)
		os.exit(1)
	end

	-- Start logging
	testing.docker_logs(config.old_container, "./logs/kellnr-old.log", true)

	-- Wait for server to start
	if not testing.wait_for_server(config.url, config.server_timeout) then
		testing.error_log("Old server did not start properly", true)
		testing.docker_stop(config.old_container)
		os.exit(1)
	end

	-- Publish crates to old version
	testing.log("Publishing crates to the old version...", true)

	-- Publish test_lib crate
	if not testing.publish_crate("crates/test_lib", config.registry) then
		testing.error_log("Failed to publish test_lib crate to old version", true)
		testing.docker_stop(config.old_container)
		os.exit(1)
	end

	-- Publish foo-bar crate
	if not testing.publish_crate("crates/foo-bar", config.registry) then
		testing.error_log("Failed to publish foo-bar crate to old version", true)
		testing.docker_stop(config.old_container)
		os.exit(1)
	end

	-- Stop old container
	testing.log("Stopping Kellnr:" .. old_image, true)
	testing.debug_log("Waiting 10 seconds before stopping...", true)
	socket.sleep(10)
	testing.docker_stop(config.old_container)

	-- Start new container
	testing.log("Starting Kellnr:" .. new_image, true)
	local run_success = testing.docker_run(config.new_container, new_image, ports, env_vars, additional_params)
	if not run_success then
		testing.error_log("Failed to start new Docker container", true)
		os.exit(1)
	end

	-- Start logging for new container
	testing.docker_logs(config.new_container, "./logs/kellnr-new.log", true)

	-- Wait for server to start
	if not testing.wait_for_server(config.url, config.server_timeout) then
		testing.error_log("New server did not start properly", true)
		testing.docker_stop(config.new_container)
		os.exit(1)
	end

	-- Publish crates to new version
	testing.log("Publishing crates to the new version...", true)

	-- Publish full-toml crate
	if not testing.publish_crate("crates/full-toml", config.registry) then
		testing.error_log("Failed to publish full-toml crate to new version", true)
		testing.docker_stop(config.new_container)
		os.exit(1)
	end

	-- Stop new container
	testing.log("Stopping Kellnr: " .. new_image, true)
	testing.docker_stop(config.new_container)

	-- Check for errors in logs
	testing.log("Checking logs for errors...", true)

	-- Check old version logs
	testing.log("Errors in Kellnr: " .. old_image .. ":", true)
	local old_errors, _ = testing.exec("grep -e ERROR logs/kellnr-old.log || true")
	print(old_errors)

	-- Check new version logs
	testing.log("Errors in Kellnr: " .. new_image .. ":", true)
	local new_errors, _ = testing.exec("grep -e ERROR logs/kellnr-new.log || true")
	print(new_errors)

	testing.log("Done", true)
end

-- Run the program
main()
