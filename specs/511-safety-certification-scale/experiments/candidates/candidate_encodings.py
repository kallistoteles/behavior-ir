"""Transform trusted generated Lean diagnostic input, not Core IR or evidence code.

The compact candidate changes chunk layout explicitly. It requires a versioned
interpretation/statement decision before any production integration.
"""
import json
import re


def compact_source(original, *, raw_literals=False, proof_only=False):
    try:
        begin=original.index('namespace ConcreteAnchors\n')
        end=original.index('end ConcreteAnchors',begin)+len('end ConcreteAnchors')
        anchors=original[begin:end]
        payloads={}
        lengths={}
        for family in ['inventory','policies']:
            leaves={name:json.loads(value) for name,value in re.findall(
                rf'^def ({family}[0-9]+) : List Nat := (\[[0-9, ]*\])$',anchors,re.M)}
            match=re.search(rf'^def {family} : List \(List Nat\) := \[([^\]]*)\]$',anchors,re.M)
            if not match:raise ValueError('Missing exact payload definition')
            names=[name.strip() for name in match[1].split(',') if name.strip()]
            if len(names)!=len(set(names)) or set(names)!=set(leaves):
                raise ValueError('Incomplete payload leaves')
            groups=[leaves[name] for name in names]
            payloads[family]=bytes(value for group in groups for value in group).decode('utf-8')
            lengths[family]=[len(group) for group in groups]
        hash_line=re.search(r'^actionHashes := (\[.*\])$',original,re.M)
        if not hash_line:raise ValueError('Missing full action keys')
        hashes=json.loads(hash_line[1])
        if not isinstance(hashes,list) or any(not isinstance(key,list) or
            any(type(value) is not int or not 0<=value<=255 for value in key) for key in hashes):
            raise ValueError('Invalid action key bytes')
        compact='namespace ConcreteAnchors\n'+''.join(
            f'def {name} : List (List Nat) := Safety.Candidate.compactPayload '+
            json.dumps(payload,ensure_ascii=False)+'\n' for name,payload in payloads.items())+'end ConcreteAnchors'
        transformed=original[:begin]+compact+original[end:]
        transformed=transformed.replace('import safety.Check\n','import safety.Check\nimport CandidateLemmas\nimport CompactPayload\n',1)
        def key_literal(key):
            if raw_literals:
                return '['+','.join(f'(nat_lit {value})' for value in key)+']'
            return json.dumps(key,separators=(',',':'))
        helpers=''.join(f'def candidateHash{i} : List Nat := '+key_literal(key)+'\n'
                        for i,key in enumerate(hashes))
        transformed=transformed.replace('def data : Safety.ModuleData :=',helpers+'def data : Safety.ModuleData :=',1)
        transformed=re.sub(r'^actionHashes := .*$',
            'actionHashes := ['+','.join(f'candidateHash{i}' for i in range(len(hashes)))+']',transformed,flags=re.M)
        if proof_only:
            transformed=re.sub(r'^def ', 'noncomputable def ', transformed, flags=re.M)
        return transformed,{'inventory_utf8_hex':payloads['inventory'].encode().hex(),
            'policies_utf8_hex':payloads['policies'].encode().hex(),'action_hashes':hashes,
            'original_chunk_lengths':lengths,'layout_change':'single interpreted UTF-8 chunk; diagnostic only'}
    except (KeyError,IndexError,TypeError,UnicodeError) as error:
        raise ValueError(f'Invalid generated diagnostic source: {error}') from error
