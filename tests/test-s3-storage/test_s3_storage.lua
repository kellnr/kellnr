#!/usr/bin/env lua

local testing = require("testing_lib")
local os = require("os")

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
	container = "kellnr-s3",
	registry = "kellnr-test",
	url = "http://localhost:8000",
	server_timeout = 60, -- seconds
	logs_dir = "logs",

	-- S3 Settings
	s3_root_user = "minioadmin",
	s3_root_password = "minioadmin",
	s3_url = "http://minio:9000",
	s3_http = "true",
	s3_image = "custom-minio",
	s3_container = "minio",
	s3_crates_bucket = "kellnr-crates",
	s3_cratesio_bucket = "kellnr-cratesio",
	s3_network = "s3-net"
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


	testing.log("Test S3 storage of Kellnr:" .. image, true)
	testing.debug_log("Debug mode is enabled", true)

	-- Create logs directory
	testing.create_directory(config.logs_dir)

	-- Create Docker network
	testing.docker_network_create(config.s3_network)

	-- Build the S3 Image
	local build_args = {
		CRATES_BUCKET = config.s3_crates_bucket,
		CRATESIO_BUCKET = config.s3_cratesio_bucket
	}
	testing.docker_build(config.s3_image, build_args, ".")

	-- Start S3 storage
	local s3_env_vars = {
		["MINIO_ROOT_USER"] = config.s3_root_user,
		["MINIO_ROOT_PASSWORD"] = config.s3_root_password
	}

	local s3_ports = { ["9000"] = "9000" }
	local s3_additional_params = "--network " .. config.s3_network

	testing.docker_run(config.s3_container, config.s3_image, s3_ports, s3_env_vars, s3_additional_params)

	-- Start Kellnr container
	local env_vars = {
		["KELLNR_LOG__LEVEL"] = "debug",
		["KELLNR_LOG__LEVEL_WEB_SERVER"] = "debug",
		["KELLNR_PROXY__ENABLED"] = "true",
		["KELLNR_S3__ENABLED"] = "true",
		["KELLNR_S3__ACCESS_KEY"] = config.s3_root_user,
		["KELLNR_S3__SECRET_KEY"] = config.s3_root_password,
		["KELLNR_S3__ENDPOINT"] = config.s3_url,
		["KELLNR_S3__ALLOW_HTTP"] = config.s3_http,
		["KELLNR_S3__CRATES_BUCKET"] = config.s3_crates_bucket,
		["KELLNR_S3__CRATESIO_BUCKET"] = config.s3_cratesio_bucket
	}

	local ports = { ["8000"] = "8000" }
	local additional_params = "--network " .. config.s3_network

	testing.docker_run(config.container, image, ports, env_vars, additional_params)

	-- Start logging
	testing.docker_logs(config.container, "./logs/kellnr-sparse.log", true)

	-- Wait for server to start
	if not testing.wait_for_server(config.url, config.server_timeout) then
		testing.error_log("Server did not start properly", true)
		testing.docker_stop(config.container)
		testing.docker_stop(config.s3_container)
		testing.docker_network_remove(config.s3_network)
		os.exit(1)
	end

	local start_time = testing.start_timer()

	-- Publish crates using the library function
	testing.publish_crate("crates/test_lib", config.registry)
	testing.publish_crate("crates/Uppercase-Name123", config.registry)
	testing.publish_crate("crates/foo-bar", config.registry)

	testing.end_timer(start_time, "Test execution time")

	-- Stop containers and clean up
	testing.log("Stopping Kellnr: " .. image, true)
	testing.docker_stop(config.container)

	testing.log("Stopping S3", true)
	testing.docker_stop(config.s3_container)

	-- Remove the network
	testing.docker_network_remove(config.s3_network)

	testing.log("Done", true)
end

-- Run the program
main()
