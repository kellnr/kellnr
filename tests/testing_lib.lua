local socket = require("socket")
local http = require("socket.http")
local ltn12 = require("ltn12")

local testing = {
	-- Default configuration
	config = {
		debug = false -- Debug logging is disabled by default
	}
}

-- ANSI color codes
local colors = {
	reset = "\027[0m",
	red = "\027[31m",
	green = "\027[32m",
	yellow = "\027[33m",
	blue = "\027[34m",
	magenta = "\027[35m",
	cyan = "\027[36m",
	white = "\027[37m"
}

-- Initialize testing library with options
function testing.init(options)
	options = options or {}
	for k, v in pairs(options) do
		testing.config[k] = v
	end
end

-- Parse command line arguments for debug flag
function testing.parse_args()
	local debug_enabled = false
	-- Use a different name for the loop variable to avoid shadowing
	for i, arg_value in ipairs(_G.arg) do
		if arg_value == "--debug" or arg_value == "-d" then
			debug_enabled = true
			break
		end
	end
	testing.init({ debug = debug_enabled })
end

-- Regular logging function
function testing.log(message, add_timestamp)
	local timestamp = ""
	if add_timestamp then
		timestamp = os.date("[%Y-%m-%d %H:%M:%S] ")
	end
	print(timestamp .. colors.green .. "INFO" .. colors.reset .. ": " .. message)
end

-- Error logging function
function testing.error_log(message, add_timestamp)
	local timestamp = ""
	if add_timestamp then
		timestamp = os.date("[%Y-%m-%d %H:%M:%S] ")
	end
	print(timestamp .. colors.red .. "ERROR" .. colors.reset .. ": " .. message)
end

-- Debug logging function - only prints if debug mode is enabled
function testing.debug_log(message, add_timestamp)
	if not testing.config.debug then
		return -- Skip debug logging if debug mode is disabled
	end

	local timestamp = ""
	if add_timestamp then
		timestamp = os.date("[%Y-%m-%d %H:%M:%S] ")
	end
	print(timestamp .. colors.yellow .. "DEBUG" .. colors.reset .. ": " .. message)
end

-- Execute a command and return output, exit code
function testing.exec(cmd)
	testing.debug_log("Executing: " .. cmd, true) -- Changed to debug_log with timestamp
	local handle = io.popen(cmd .. " 2>&1 ; echo $?")
	local result = handle:read("*a")
	handle:close()

	-- Extract exit code from the last line
	local output = result:match("(.*)%s*(%d+)%s*$")
	local exit_code = tonumber(result:match(".*%s*(%d+)%s*$"))

	return output, exit_code == 0
end

-- Execute a command, log results, and optionally exit on failure
function testing.exec_with_logging(cmd, exit_on_failure)
	local output, success = testing.exec(cmd)

	if not success then
		testing.error_log("Command failed: " .. cmd, true)
		testing.error_log("Output: " .. output, false)

		if exit_on_failure then
			os.exit(1)
		end
	end

	return output, success
end

-- Check HTTP status for a URL
function testing.check_http_status(url)
	local response = {}
	local _, status = http.request {
		url = url,
		sink = ltn12.sink.table(response),
		redirect = false,
		timeout = 5
	}
	return status
end

-- Wait for server to become available with timeout
function testing.wait_for_server(url, timeout, expected_status)
	local start_time = os.time()
	expected_status = expected_status or 200

	testing.log("Waiting for server at " .. url .. " to return status " .. expected_status, true)

	while true do
		local status = testing.check_http_status(url)

		if status == expected_status then
			testing.log("Server is ready (status " .. status .. ")", true)
			return true
		end

		if os.time() - start_time > timeout then
			testing.error_log("Server did not return status " .. expected_status ..
				" within timeout period (" .. timeout .. "s)", true)
			return false
		end

		socket.sleep(1)
	end
end

-- Create directory if it doesn't exist
function testing.create_directory(path)
	local success = os.execute("mkdir -p " .. path)
	if not success then
		testing.error_log("Failed to create directory: " .. path, true)
		return false
	end
	return true
end

-- Docker network management functions
function testing.docker_network_create(network_name)
	testing.log("Creating Docker network: " .. network_name, true)
	return testing.exec_with_logging("docker network create " .. network_name, false)
end

function testing.docker_network_remove(network_name)
	testing.log("Removing Docker network: " .. network_name, true)
	return testing.exec_with_logging("docker network rm " .. network_name, false)
end

-- Docker build function
function testing.docker_build(tag, build_args, context)
	context = context or "."
	local cmd = "docker build -t " .. tag

	-- Add build args if provided
	if build_args and type(build_args) == "table" then
		for key, value in pairs(build_args) do
			cmd = cmd .. " --build-arg " .. key .. "=" .. value
		end
	end

	cmd = cmd .. " " .. context

	testing.log("Building Docker image: " .. tag, true)
	return testing.exec_with_logging(cmd, false)
end

-- Docker pull function
function testing.docker_pull(image)
	testing.log("Pulling Docker image: " .. image, true)
	return testing.exec_with_logging("docker pull " .. image, false)
end

-- Docker run function
function testing.docker_run(container_name, image, ports, env_vars, additional_params)
	testing.log("Starting container: " .. container_name, true)

	local cmd = "docker run --rm --name " .. container_name

	-- Add port mappings
	for host_port, container_port in pairs(ports or {}) do
		cmd = cmd .. " -p " .. host_port .. ":" .. container_port
	end

	-- Add environment variables
	for key, value in pairs(env_vars or {}) do
		cmd = cmd .. " -e \"" .. key .. "=" .. value .. "\""
	end

	-- Add any additional parameters
	if additional_params then
		cmd = cmd .. " " .. additional_params
	end

	-- Add image name
	cmd = cmd .. " -d " .. image

	return testing.exec_with_logging(cmd, false)
end

-- Docker stop function
function testing.docker_stop(container_name)
	testing.log("Stopping container: " .. container_name, true)
	return testing.exec_with_logging("docker stop " .. container_name, false)
end

-- Docker logs function
function testing.docker_logs(container_name, output_file, strip_colors)
	local cmd

	if strip_colors then
		cmd = "docker logs -f " .. container_name ..
		    " | lua -e \"" ..
		    "while true do " ..
		    "  local line = io.read() " ..
		    "  if not line then break end " ..
		    "  line = line:gsub('\\027%[[%d;]+m', '') " .. -- Strip ANSI colors
		    "  io.stdout:write(line .. '\\n') " ..
		    "end" ..
		    "\""
	else
		cmd = "docker logs -f " .. container_name
	end

	if output_file then
		cmd = cmd .. " > " .. output_file .. " &"
	else
		cmd = cmd .. " &"
	end

	return testing.exec_with_logging(cmd, false)
end

-- Function to publish a Cargo crate
function testing.publish_crate(crate_path, registry, options)
	testing.log("Publishing crate at " .. crate_path, true)
	options = options or {}

	-- Remove Cargo.lock if requested (default is true)
	if options.remove_lock ~= false then
		os.remove(crate_path .. "/Cargo.lock")
	end

	-- Build the cargo publish command
	local cmd = "cargo"

	-- Add toolchain if specified
	if options.toolchain then
		cmd = cmd .. " +" .. options.toolchain
	end

	-- Add the publish command and registry
	cmd = cmd .. " publish --registry " .. registry

	-- Add allow-dirty if requested (default is true)
	if options.allow_dirty ~= false then
		cmd = cmd .. " --allow-dirty"
	end

	-- Add any additional options
	if options.additional_args then
		cmd = cmd .. " " .. options.additional_args
	end

	-- Execute in the target directory
	local full_cmd = "cd " .. crate_path .. " && " .. cmd

	return testing.exec_with_logging(full_cmd, false)
end

-- Timing utilities
function testing.start_timer()
	return os.time()
end

function testing.end_timer(start_time, message)
	local duration = os.time() - start_time
	local msg = message or "Execution time"
	testing.log(msg .. ": " .. duration .. " seconds", true)
	return duration
end

-- Helper function to format date as UTC
function testing.format_utc_date(timestamp)
	timestamp = timestamp or os.time()
	return os.date("!%Y-%m-%d %H:%M:%S", timestamp) -- '!' prefix for UTC
end

-- Parse command line arguments right away
testing.parse_args()

return testing
