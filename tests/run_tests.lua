#!/usr/bin/env lua

local testing = require("testing_lib")
local os = require("os")
local socket = require("socket")

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

-- Main function
local function main()
	-- Ensure the version argument is provided
	local new_version = arg[1]
	if not new_version then
		testing.log("Usage: " .. arg[0] .. " <new-version>", true)
		testing.log("New version has to be a version from the Raspi registry.", true)
		os.exit(1)
	end

	-- Fetch the latest version of Kellnr from GitHub
	local latest_version_cmd =
	"curl -sL https://api.github.com/repos/kellnr/kellnr/releases/latest | jq -r \".tag_name\" | cut -c 2-"
	local handle = io.popen(latest_version_cmd)
	local latest_version = handle:read("*a"):gsub("%s+$", "")
	handle:close()

	if latest_version == "" then
		testing.error_log("Failed to fetch the latest released version of Kellnr.", true)
		os.exit(1)
	end

	testing.log("INFO: Latest released version of Kellnr is: " .. latest_version, true)

	-- Run each test script
	for _, test in ipairs(test_scripts) do
		run_test(test.dir, test.script, new_version, latest_version, test.description, test.requires_latest_version)
	end

	testing.log("--- ALL TESTS FINISHED ---", true)
end

-- Run the script
main()
