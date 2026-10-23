"""Public authoring fixtures with expectations independent of Safety extraction."""
from pathlib import Path
import sys

import pytest

EXPERIMENTS = Path(__file__).resolve().parents[1]
sys.path.insert(0,str(EXPERIMENTS))
from workloads import build_case, describe_case
from certification_probe import make_fo


def inventory(model):
    a = model.safety_analysis.to_dict()
    assert a['issues'] == []
    return a


@pytest.mark.parametrize('size',[256,512])
def test_mandatory_references_preserve_exact_original_modules(size):
    model, expected = build_case({'kind':'fo','actions':size,'operations':1,'policies':1})
    original, original_expected = make_fo(size)
    assert model.to_wire_json() == original.to_wire_json()
    assert model.behavior_version == original.behavior_version
    assert expected == original_expected
    description = describe_case(model,expected)
    assert description['actions'] == size
    assert description['operations'] == size
    assert description['ownership_policies'] == 1
    assert description['lifecycle_dependencies'] == 0
    assert description['encoded_inventory_bytes'] > 0


def test_fo_operation_and_policy_sweeps_keep_counts_independent():
    for policies in [1,2,4]:
        model, expected = build_case({'kind':'fo','actions':3,'operations':4,'policies':policies})
        d=describe_case(model,expected)
        assert (d['actions'],d['operations'],d['ownership_policies']) == (3,12,policies)
    with pytest.raises(ValueError):
        build_case({'kind':'fo','actions':3,'operations':1,'policies':2})


@pytest.mark.parametrize('dependencies',[1,2,4])
@pytest.mark.parametrize('guard_width',[1,3])
def test_lifecycle_union_inventory_and_guard_dimensions(dependencies,guard_width):
    model, expected = build_case({'kind':'lifecycle','dependencies':dependencies,'guard_width':guard_width})
    d=describe_case(model,expected)
    assert d['actions'] == 4*dependencies+4
    assert d['operations'] == 4*dependencies+4
    assert d['ownership_policies'] == dependencies+1
    assert d['lifecycle_dependencies'] == dependencies
    assert d['required_precondition_roots'] == 4*dependencies
    assert d['guard_literal_width'] == guard_width


def test_mixed_generated_and_authored_actions_are_all_expected():
    model, expected = build_case({'kind':'lifecycle','dependencies':2,'guard_width':2,
                                  'generated':True,'fo_actions':3})
    d=describe_case(model,expected)
    assert d['actions'] == 16
    assert d['operations'] == 16
    assert d['ownership_policies'] == 5
    assert d['lifecycle_dependencies'] == 2
    assert any(name.endswith('__finish') for name in expected)
    assert model.model_receipts


def test_encoded_size_axis_and_determinism():
    left,e = build_case({'kind':'fo','actions':2,'padding':0})
    right,f = build_case({'kind':'fo','actions':2,'padding':64})
    duplicate,g = build_case({'kind':'fo','actions':2,'padding':64})
    ld,rd=describe_case(left,e),describe_case(right,f)
    assert ld['actions'] == rd['actions'] == 2
    assert rd['encoded_inventory_bytes'] > ld['encoded_inventory_bytes']
    assert right.behavior_version == duplicate.behavior_version
    assert f == g


def test_expected_occurrences_cannot_be_replaced_by_extracted_counts():
    model,expected = build_case({'kind':'fo','actions':2})
    expected.pop(next(iter(expected)))
    with pytest.raises(ValueError,match='inventory'):
        describe_case(model,expected)


@pytest.mark.parametrize('case_config',[{'kind':'import','module':'os'}, {'kind':'fo','actions':0},
    {'kind':'fo','operations':0},{'kind':'lifecycle','dependencies':0},
    {'kind':'lifecycle','guard_width':0},{'kind':'fo','actions':True}])
def test_invalid_or_executable_case_configuration_is_rejected(case_config):
    with pytest.raises(ValueError):
        build_case(case_config)
