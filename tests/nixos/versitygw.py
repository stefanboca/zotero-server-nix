from typing import TYPE_CHECKING

if TYPE_CHECKING:
    from test_driver.machine import BaseMachine


def run(machine: "BaseMachine") -> None:
    machine.wait_for_unit("versitygw.service")
    machine.wait_for_open_port(10000)
    unit = machine.succeed("systemctl cat versitygw.service")
    assert "LoadCredential=root-access-key-id:" in unit, unit
    assert "LoadCredential=root-secret-access-key:" in unit, unit
    assert "EnvironmentFile=" not in unit, unit
    command_line = machine.succeed(
        "xargs --null --max-args=1 < /proc/"
        "$(systemctl show -P MainPID versitygw.service)/cmdline"
    ).splitlines()
    assert "--disableotmp" in command_line, command_line
    assert "--disable-copy-file-range" in command_line, command_line
    assert "/var/lib/versitygw-sidecar" in command_line, command_line
    assert "/srv/versitygw-test" in command_line, command_line
    assert (
        machine.succeed("stat --format='%U:%G:%a' /var/lib/versitygw-sidecar").strip()
        == "versitygw:versitygw:750"
    )
    machine.succeed("curl --silent --show-error --fail http://127.0.0.1:10000/health")

    def s3(method, path, options=""):
        return machine.succeed(
            "curl --silent --show-error --fail-with-body "
            "--aws-sigv4 'aws:amz:us-east-1:s3' "
            "--user 'myaccess:mysecret' "
            f"--request {method} {options} "
            f"http://127.0.0.1:10000/{path}"
        )

    s3("PUT", "zotero")
    s3("PUT", "zotero/attachment.txt", "--data-binary 'attachment data'")
    assert s3("GET", "zotero/attachment.txt") == "attachment data"

    machine.succeed("systemctl restart versitygw.service")
    machine.wait_for_open_port(10000)
    assert s3("GET", "zotero/attachment.txt") == "attachment data"
